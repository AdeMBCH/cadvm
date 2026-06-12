Minimal STEP/STP fixtures used by the integration tests.

- cube_hole5.step  : cube with a 5 mm hole (radius 2.5)
- cube_hole8.step  : same cube with an 8 mm hole (radius 4.0) plus an extra point
- two_holes.stp    : plate with two 5 mm holes (also exercises the .stp extension)

This non-.step/.stp file is intentionally present so tests can assert that
cadvm tracks only STEP/STP files.
