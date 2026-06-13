// cadvm-geom — geometric STEP helper for cadvm (Step 2/3).
//
// A standalone C++/Open CASCADE executable, invoked as a subprocess by the Rust
// core. Two subcommands:
//
//   diff <a.step> <b.step>
//       Boolean decomposition of two shapes, printed as JSON to stdout:
//         added   = B - A,  removed = A - B,  common = A ∩ B
//       plus per-input metrics (volume, area, solids/shells/faces, bbox).
//
//   mesh <a.step> <b.step> <out.json>
//       Tessellates the three boolean pieces and writes their triangle meshes
//       (positions + flat normals) to <out.json>, for the 3D viewer.
//
// Handled geometry failures print `{"status":"error",...}` and exit 0 so the
// caller always gets structured output. Usage errors exit 2.

#include <algorithm>
#include <cmath>
#include <fstream>
#include <iostream>
#include <map>
#include <sstream>
#include <stdexcept>
#include <string>
#include <vector>

#include <BRepAdaptor_Surface.hxx>
#include <BRepAlgoAPI_Common.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepBndLib.hxx>
#include <BRepGProp.hxx>
#include <BRepMesh_IncrementalMesh.hxx>
#include <BRep_Tool.hxx>
#include <Bnd_Box.hxx>
#include <GProp_GProps.hxx>
#include <IFSelect_ReturnStatus.hxx>
#include <Poly_Triangulation.hxx>
#include <STEPControl_Reader.hxx>
#include <Standard_Failure.hxx>
#include <TopAbs_ShapeEnum.hxx>
#include <TopExp_Explorer.hxx>
#include <TopLoc_Location.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Face.hxx>
#include <TopoDS_Shape.hxx>
#include <gp_Cone.hxx>
#include <gp_Cylinder.hxx>
#include <gp_Dir.hxx>
#include <gp_Pln.hxx>
#include <gp_Pnt.hxx>
#include <gp_Sphere.hxx>
#include <gp_Torus.hxx>
#include <gp_Trsf.hxx>
#include <gp_Vec.hxx>

namespace {

TopoDS_Shape read_step(const std::string& path) {
    STEPControl_Reader reader;
    IFSelect_ReturnStatus status = reader.ReadFile(path.c_str());
    if (status != IFSelect_RetDone) {
        throw std::runtime_error("failed to read STEP file: " + path);
    }
    reader.TransferRoots();
    TopoDS_Shape shape = reader.OneShape();
    if (shape.IsNull()) {
        throw std::runtime_error("STEP file produced no shape: " + path);
    }
    return shape;
}

double volume(const TopoDS_Shape& shape) {
    if (shape.IsNull()) return 0.0;
    GProp_GProps props;
    BRepGProp::VolumeProperties(shape, props);
    return std::fabs(props.Mass());
}

double area(const TopoDS_Shape& shape) {
    if (shape.IsNull()) return 0.0;
    GProp_GProps props;
    BRepGProp::SurfaceProperties(shape, props);
    return props.Mass();
}

int count(const TopoDS_Shape& shape, TopAbs_ShapeEnum kind) {
    int n = 0;
    for (TopExp_Explorer it(shape, kind); it.More(); it.Next()) ++n;
    return n;
}

TopoDS_Shape boolean_cut(const TopoDS_Shape& a, const TopoDS_Shape& b) {
    BRepAlgoAPI_Cut op(a, b);
    op.Build();
    if (!op.IsDone()) throw std::runtime_error("boolean Cut failed");
    return op.Shape();
}

TopoDS_Shape boolean_common(const TopoDS_Shape& a, const TopoDS_Shape& b) {
    BRepAlgoAPI_Common op(a, b);
    op.Build();
    if (!op.IsDone()) throw std::runtime_error("boolean Common failed");
    return op.Shape();
}

std::string json_escape(const std::string& s) {
    std::ostringstream out;
    for (char c : s) {
        switch (c) {
            case '"': out << "\\\""; break;
            case '\\': out << "\\\\"; break;
            case '\n': out << "\\n"; break;
            case '\r': out << "\\r"; break;
            case '\t': out << "\\t"; break;
            default: out << c;
        }
    }
    return out.str();
}

// ---- topological (face-to-face) diff --------------------------------------

// Round a length to microns / a direction component to 1e-5, as integers.
static long long rl(double x) { return static_cast<long long>(std::llround(x * 1000.0)); }
static long long rd(double x) { return static_cast<long long>(std::llround(x * 100000.0)); }

// Is (x,y,z) in the canonical hemisphere (first significant component positive)?
// Used to give a surface and its reverse the same orientation-independent key.
static bool canonical(double x, double y, double z) {
    const double e = 1e-7;
    if (x > e) return true;
    if (x < -e) return false;
    if (y > e) return true;
    if (y < -e) return false;
    return z >= 0;
}

// A signature of a face's *underlying surface* (plane equation, cylinder axis +
// radius, …), invariant to how the face is trimmed. Two faces lying on the same
// surface match even if one was cut by a new feature — so a wall that merely
// gets a hole stays "unchanged", and only genuinely new/removed surfaces (e.g.
// the hole's cylinder) show up as added/removed. Freeform surfaces fall back to
// area + centroid.
std::string face_signature(const TopoDS_Face& f) {
    BRepAdaptor_Surface s(f);
    std::ostringstream o;
    switch (s.GetType()) {
        case GeomAbs_Plane: {
            const gp_Pln pl = s.Plane();
            const gp_Dir n = pl.Axis().Direction();
            const gp_Pnt l = pl.Location();
            double nx = n.X(), ny = n.Y(), nz = n.Z();
            double d = nx * l.X() + ny * l.Y() + nz * l.Z();  // signed distance to origin
            if (!canonical(nx, ny, nz)) {
                nx = -nx;
                ny = -ny;
                nz = -nz;
                d = -d;
            }
            o << "plane:" << rd(nx) << "," << rd(ny) << "," << rd(nz) << "," << rl(d);
            break;
        }
        case GeomAbs_Cylinder: {
            const gp_Cylinder cy = s.Cylinder();
            const gp_Dir dir = cy.Axis().Direction();
            const gp_Pnt l = cy.Axis().Location();
            double dx = dir.X(), dy = dir.Y(), dz = dir.Z();
            // Canonical point: foot of perpendicular from the origin onto the axis.
            const double t = -(l.X() * dx + l.Y() * dy + l.Z() * dz);
            const double px = l.X() + t * dx, py = l.Y() + t * dy, pz = l.Z() + t * dz;
            if (!canonical(dx, dy, dz)) {
                dx = -dx;
                dy = -dy;
                dz = -dz;
            }
            o << "cyl:" << rd(dx) << "," << rd(dy) << "," << rd(dz) << "," << rl(px) << "," << rl(py)
              << "," << rl(pz) << "," << rl(cy.Radius());
            break;
        }
        case GeomAbs_Sphere: {
            const gp_Sphere sp = s.Sphere();
            const gp_Pnt c = sp.Location();
            o << "sph:" << rl(c.X()) << "," << rl(c.Y()) << "," << rl(c.Z()) << "," << rl(sp.Radius());
            break;
        }
        case GeomAbs_Cone: {
            const gp_Cone cn = s.Cone();
            const gp_Dir dir = cn.Axis().Direction();
            const gp_Pnt a = cn.Apex();
            double dx = dir.X(), dy = dir.Y(), dz = dir.Z();
            if (!canonical(dx, dy, dz)) {
                dx = -dx;
                dy = -dy;
                dz = -dz;
            }
            o << "cone:" << rd(dx) << "," << rd(dy) << "," << rd(dz) << "," << rl(a.X()) << ","
              << rl(a.Y()) << "," << rl(a.Z()) << "," << rl(cn.SemiAngle());
            break;
        }
        case GeomAbs_Torus: {
            const gp_Torus tr = s.Torus();
            const gp_Dir dir = tr.Axis().Direction();
            const gp_Pnt c = tr.Location();
            double dx = dir.X(), dy = dir.Y(), dz = dir.Z();
            if (!canonical(dx, dy, dz)) {
                dx = -dx;
                dy = -dy;
                dz = -dz;
            }
            o << "tor:" << rl(c.X()) << "," << rl(c.Y()) << "," << rl(c.Z()) << "," << rd(dx) << ","
              << rd(dy) << "," << rd(dz) << "," << rl(tr.MajorRadius()) << "," << rl(tr.MinorRadius());
            break;
        }
        default: {
            // Freeform (BSpline/Bezier/…): coarse fallback on area + centroid.
            GProp_GProps props;
            BRepGProp::SurfaceProperties(f, props);
            const gp_Pnt c = props.CentreOfMass();
            o << "free:" << static_cast<int>(s.GetType()) << ":" << rl(props.Mass()) << ":"
              << rl(c.X()) << ":" << rl(c.Y()) << ":" << rl(c.Z());
        }
    }
    return o.str();
}

void face_histogram(const TopoDS_Shape& s, std::map<std::string, int>& hist) {
    for (TopExp_Explorer it(s, TopAbs_FACE); it.More(); it.Next()) {
        hist[face_signature(TopoDS::Face(it.Current()))]++;
    }
}

struct FaceTopo {
    long long common = 0, added = 0, removed = 0;
};

FaceTopo face_topo(const TopoDS_Shape& a, const TopoDS_Shape& b) {
    std::map<std::string, int> ha, hb;
    face_histogram(a, ha);
    face_histogram(b, hb);
    FaceTopo t;
    for (const auto& kv : ha) {
        const auto it = hb.find(kv.first);
        const int nb = (it == hb.end()) ? 0 : it->second;
        const int common = std::min(kv.second, nb);
        t.common += common;
        t.removed += kv.second - common;
    }
    for (const auto& kv : hb) {
        const auto it = ha.find(kv.first);
        const int na = (it == ha.end()) ? 0 : it->second;
        t.added += kv.second - std::min(kv.second, na);
    }
    return t;
}

// ---- diff -----------------------------------------------------------------

std::string piece_json(const TopoDS_Shape& s) {
    std::ostringstream out;
    out << "{\"volume\":" << volume(s) << ",\"faces\":" << count(s, TopAbs_FACE) << "}";
    return out.str();
}

std::string shape_json(const TopoDS_Shape& s) {
    std::ostringstream out;
    out << "{\"volume\":" << volume(s) << ",\"area\":" << area(s)
        << ",\"solids\":" << count(s, TopAbs_SOLID) << ",\"shells\":" << count(s, TopAbs_SHELL)
        << ",\"faces\":" << count(s, TopAbs_FACE) << ",\"bbox\":";
    Bnd_Box box;
    BRepBndLib::Add(s, box);
    if (box.IsVoid()) {
        out << "null";
    } else {
        Standard_Real xmin, ymin, zmin, xmax, ymax, zmax;
        box.Get(xmin, ymin, zmin, xmax, ymax, zmax);
        out << "{\"min\":[" << xmin << "," << ymin << "," << zmin << "],\"max\":[" << xmax << ","
            << ymax << "," << zmax << "]}";
    }
    out << "}";
    return out.str();
}

int run_diff(const std::string& file_a, const std::string& file_b) {
    TopoDS_Shape a = read_step(file_a);
    TopoDS_Shape b = read_step(file_b);
    const TopoDS_Shape common = boolean_common(a, b);
    const TopoDS_Shape removed = boolean_cut(a, b);
    const TopoDS_Shape added = boolean_cut(b, a);
    const FaceTopo ft = face_topo(a, b);

    std::cout << "{"
              << "\"status\":\"ok\","
              << "\"file_a\":\"" << json_escape(file_a) << "\","
              << "\"file_b\":\"" << json_escape(file_b) << "\","
              << "\"a\":" << shape_json(a) << ","
              << "\"b\":" << shape_json(b) << ","
              << "\"common\":" << piece_json(common) << ","
              << "\"added\":" << piece_json(added) << ","
              << "\"removed\":" << piece_json(removed) << ","
              << "\"faces_topo\":{\"common\":" << ft.common << ",\"added\":" << ft.added
              << ",\"removed\":" << ft.removed << "}}\n";
    return 0;
}

// ---- mesh -----------------------------------------------------------------

// Flat-shaded triangle soup: 9 floats (3 verts) per triangle in `positions`,
// with the matching per-vertex (face) normal in `normals`.
struct Mesh {
    std::vector<float> positions;
    std::vector<float> normals;
};

// Run OCCT's mesher over a shape so every face gets a triangulation.
void mesh_shape(const TopoDS_Shape& shape) {
    if (shape.IsNull()) return;
    // Linear/angular deflection tuned for typical mm-scale parts.
    BRepMesh_IncrementalMesh mesher(shape, 0.4, Standard_False, 0.4, Standard_True);
    mesher.Perform();
}

// Append one (already-meshed) face's triangles to `mesh`, flat-shaded.
void append_face(const TopoDS_Face& face, Mesh& mesh) {
    TopLoc_Location loc;
    Handle(Poly_Triangulation) tri = BRep_Tool::Triangulation(face, loc);
    if (tri.IsNull()) return;
    const gp_Trsf& trsf = loc.Transformation();
    const bool reversed = (face.Orientation() == TopAbs_REVERSED);

    for (Standard_Integer i = 1; i <= tri->NbTriangles(); ++i) {
        Standard_Integer n1, n2, n3;
        tri->Triangle(i).Get(n1, n2, n3);
        if (reversed) std::swap(n2, n3);
        const gp_Pnt p1 = tri->Node(n1).Transformed(trsf);
        const gp_Pnt p2 = tri->Node(n2).Transformed(trsf);
        const gp_Pnt p3 = tri->Node(n3).Transformed(trsf);

        gp_Vec normal(gp_Vec(p1, p2).Crossed(gp_Vec(p1, p3)));
        if (normal.Magnitude() > 1e-12) normal.Normalize();

        const gp_Pnt pts[3] = {p1, p2, p3};
        for (const gp_Pnt& p : pts) {
            mesh.positions.push_back(static_cast<float>(p.X()));
            mesh.positions.push_back(static_cast<float>(p.Y()));
            mesh.positions.push_back(static_cast<float>(p.Z()));
            mesh.normals.push_back(static_cast<float>(normal.X()));
            mesh.normals.push_back(static_cast<float>(normal.Y()));
            mesh.normals.push_back(static_cast<float>(normal.Z()));
        }
    }
}

void write_floats(std::ostream& out, const std::vector<float>& v) {
    out << "[";
    for (size_t i = 0; i < v.size(); ++i) {
        if (i) out << ",";
        out << v[i];
    }
    out << "]";
}

void write_mesh(std::ostream& out, const Mesh& m) {
    out << "{\"positions\":";
    write_floats(out, m.positions);
    out << ",\"normals\":";
    write_floats(out, m.normals);
    out << "}";
}

int run_mesh(const std::string& file_a, const std::string& file_b, const std::string& out_path) {
    TopoDS_Shape a = read_step(file_a);
    TopoDS_Shape b = read_step(file_b);
    mesh_shape(a);
    mesh_shape(b);

    // Per-face classification by geometric signature: a face of B that matches a
    // face of A is "unchanged"; otherwise it is "added". A face of A with no
    // match in B is "removed". This colors the real faces of the part rather
    // than abstract boolean solids.
    Mesh m_unchanged, m_added, m_removed;

    std::map<std::string, int> budget_a;
    face_histogram(a, budget_a);
    for (TopExp_Explorer ex(b, TopAbs_FACE); ex.More(); ex.Next()) {
        const TopoDS_Face face = TopoDS::Face(ex.Current());
        const std::string sig = face_signature(face);
        auto it = budget_a.find(sig);
        if (it != budget_a.end() && it->second > 0) {
            it->second--;
            append_face(face, m_unchanged);
        } else {
            append_face(face, m_added);
        }
    }

    std::map<std::string, int> budget_b;
    face_histogram(b, budget_b);
    for (TopExp_Explorer ex(a, TopAbs_FACE); ex.More(); ex.Next()) {
        const TopoDS_Face face = TopoDS::Face(ex.Current());
        const std::string sig = face_signature(face);
        auto it = budget_b.find(sig);
        if (it != budget_b.end() && it->second > 0) {
            it->second--;  // common face, already shown via B's "unchanged"
        } else {
            append_face(face, m_removed);
        }
    }

    std::ofstream out(out_path);
    if (!out) throw std::runtime_error("cannot open output file: " + out_path);

    out << "{\"status\":\"ok\",\"bbox\":";
    Bnd_Box box;
    BRepBndLib::Add(a, box);
    BRepBndLib::Add(b, box);
    if (box.IsVoid()) {
        out << "null";
    } else {
        Standard_Real xmin, ymin, zmin, xmax, ymax, zmax;
        box.Get(xmin, ymin, zmin, xmax, ymax, zmax);
        out << "{\"min\":[" << xmin << "," << ymin << "," << zmin << "],\"max\":[" << xmax << ","
            << ymax << "," << zmax << "]}";
    }
    out << ",\"layers\":{\"unchanged\":";
    write_mesh(out, m_unchanged);
    out << ",\"added\":";
    write_mesh(out, m_added);
    out << ",\"removed\":";
    write_mesh(out, m_removed);
    out << "}}\n";

    // A short stdout acknowledgement (the data is in the file).
    std::cout << "{\"status\":\"ok\",\"out\":\"" << json_escape(out_path) << "\"}\n";
    return 0;
}

void print_error(const std::string& msg) {
    std::cout << "{\"status\":\"error\",\"error\":\"" << json_escape(msg) << "\"}\n";
}

}  // namespace

int main(int argc, char** argv) {
    const std::string cmd = argc > 1 ? argv[1] : "";

    try {
        if (cmd == "diff" && argc == 4) {
            return run_diff(argv[2], argv[3]);
        }
        if (cmd == "mesh" && argc == 5) {
            return run_mesh(argv[2], argv[3], argv[4]);
        }
        std::cerr << "usage:\n"
                  << "  cadvm-geom diff <a.step> <b.step>\n"
                  << "  cadvm-geom mesh <a.step> <b.step> <out.json>\n";
        return 2;
    } catch (const Standard_Failure& e) {
        print_error(std::string("OCCT: ") + e.GetMessageString());
        return 0;
    } catch (const std::exception& e) {
        print_error(e.what());
        return 0;
    }
}
