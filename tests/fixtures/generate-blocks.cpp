// Generate the demo fixtures: two versions of the same part so the per-face
// diff is easy to read (mostly unchanged, with one clear added feature).
//
//   block_v1.step : a 40x30x20 block with a Ø10 through-hole near the left.
//   block_v2.step : the hole moved to the right, plus a raised boss on top.
// With exact (surface-based) face matching this reads as: the block body is
// unchanged (grey), the old hole is removed (red), and the new hole and the boss
// are added (green) — one change inside the part, one clearly on its surface.
//
// Not part of the build. Compile manually (needs Open CASCADE):
//   g++ -std=c++17 -I/usr/include/opencascade tests/fixtures/generate-blocks.cpp \
//       -o /tmp/genblocks -lTKSTEP -lTKXSBase -lTKBRep -lTKMath -lTKG2d -lTKG3d \
//       -lTKGeomBase -lTKGeomAlgo -lTKTopAlgo -lTKPrim -lTKBO -lTKernel
//   /tmp/genblocks tests/fixtures/block_v1.step tests/fixtures/block_v2.step

#include <stdexcept>
#include <string>

#include <BRepAlgoAPI_Cut.hxx>
#include <BRepAlgoAPI_Fuse.hxx>
#include <BRepMesh_IncrementalMesh.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <BRepPrimAPI_MakeCylinder.hxx>
#include <STEPControl_Writer.hxx>
#include <StlAPI_Writer.hxx>
#include <TopoDS_Shape.hxx>
#include <gp_Ax2.hxx>
#include <gp_Dir.hxx>
#include <gp_Pnt.hxx>

static void write_step(const TopoDS_Shape& shape, const std::string& path) {
    STEPControl_Writer writer;
    if (writer.Transfer(shape, STEPControl_AsIs) != IFSelect_RetDone) {
        throw std::runtime_error("STEP transfer failed for " + path);
    }
    if (writer.Write(path.c_str()) != IFSelect_RetDone) {
        throw std::runtime_error("STEP write failed for " + path);
    }
}

static void write_stl(const TopoDS_Shape& shape, const std::string& path) {
    BRepMesh_IncrementalMesh mesher(shape, 0.4, Standard_False, 0.4, Standard_True);
    mesher.Perform();
    StlAPI_Writer writer;
    if (!writer.Write(shape, path.c_str())) {
        throw std::runtime_error("STL write failed for " + path);
    }
}

static TopoDS_Shape hole(double cx, double cy) {
    // Ø10 vertical cylinder, taller than the block so it cuts all the way.
    gp_Ax2 axis(gp_Pnt(cx, cy, -1.0), gp_Dir(0.0, 0.0, 1.0));
    return BRepPrimAPI_MakeCylinder(axis, 5.0, 22.0).Shape();
}

int main(int argc, char** argv) {
    // Usage: genblocks v1.step v2.step [v1.stl v2.stl]
    if (argc != 3 && argc != 5) {
        return 2;
    }
    const TopoDS_Shape block = BRepPrimAPI_MakeBox(40.0, 30.0, 20.0).Shape();

    // v1: one hole near the left end.
    const TopoDS_Shape v1 = BRepAlgoAPI_Cut(block, hole(12.0, 15.0));

    // v2: hole moved to the right end, plus a Ø12 x 8 boss on top.
    const TopoDS_Shape v2_holed = BRepAlgoAPI_Cut(block, hole(28.0, 15.0));
    gp_Ax2 boss_axis(gp_Pnt(14.0, 15.0, 20.0), gp_Dir(0.0, 0.0, 1.0));
    const TopoDS_Shape boss = BRepPrimAPI_MakeCylinder(boss_axis, 6.0, 8.0).Shape();
    const TopoDS_Shape v2 = BRepAlgoAPI_Fuse(v2_holed, boss);

    write_step(v1, argv[1]);
    write_step(v2, argv[2]);
    if (argc == 5) {
        write_stl(v1, argv[3]);
        write_stl(v2, argv[4]);
    }
    return 0;
}
