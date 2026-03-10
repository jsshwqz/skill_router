use std::fs::File;
use std::io::Write;
use std::f64::consts::PI;

pub fn generate_teacup_files() -> std::io::Result<()> {
    let radius = 25.0;
    let height = 120.0;
    let segments = 36;

    // 1. Generate STL (ASCII Format)
    let mut stl_file = File::create("teacup_50x120_v3.stl")?;
    writeln!(stl_file, "solid teacup")?;
    
    let mut circle = Vec::new();
    for i in 0..segments {
        let angle = 2.0 * PI * (i as f64) / (segments as f64);
        circle.push((radius * angle.cos(), radius * angle.sin()));
    }

    let write_facet = |f: &mut File, v1: (f64,f64,f64), v2: (f64,f64,f64), v3: (f64,f64,f64)| -> std::io::Result<()> {
        writeln!(f, "  facet normal 0 0 0
    outer loop")?;
        writeln!(f, "      vertex {:.4} {:.4} {:.4}", v1.0, v1.1, v1.2)?;
        writeln!(f, "      vertex {:.4} {:.4} {:.4}", v2.0, v2.1, v2.2)?;
        writeln!(f, "      vertex {:.4} {:.4} {:.4}", v3.0, v3.1, v3.2)?;
        writeln!(f, "    endloop
  endfacet")
    };

    // Bottom and Sides
    for i in 0..segments {
        let next = (i + 1) % segments;
        // Bottom
        write_facet(&mut stl_file, (0.0,0.0,0.0), (circle[next].0, circle[next].1, 0.0), (circle[i].0, circle[i].1, 0.0))?;
        // Side 1
        write_facet(&mut stl_file, (circle[i].0, circle[i].1, 0.0), (circle[next].0, circle[next].1, 0.0), (circle[i].0, circle[i].1, height))?;
        // Side 2
        write_facet(&mut stl_file, (circle[next].0, circle[next].1, 0.0), (circle[next].0, circle[next].1, height), (circle[i].0, circle[i].1, height))?;
        // Top
        write_facet(&mut stl_file, (0.0,0.0,height), (circle[i].0, circle[i].1, height), (circle[next].0, circle[next].1, height))?;
    }
    writeln!(stl_file, "endsolid teacup")?;

    // 2. Generate DXF (Minimal R12)
    let mut dxf_file = File::create("teacup_50x120_v3.dxf")?;
    writeln!(dxf_file, "0
SECTION
2
ENTITIES
0
CIRCLE
8
0
10
0.0
20
0.0
30
0.0
40
25.0
0
ENDSEC
0
EOF")?;

    println!("Generated: teacup_50x120_v3.stl (3D) and teacup_50x120_v3.dxf (CAD)");
    Ok(())
}
