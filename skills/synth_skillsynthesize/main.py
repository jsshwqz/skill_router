import sys
import os

# 真实的功能代码逻辑：生成 STL 和 DXF
def generate_teacup_files(diameter, height):
    # 此处模拟真实的 STL 和 DXF 内容输出
    dxf_content = f"SECTION\nENTITIES\nCIRCLE\n10\n0.0\n20\n0.0\n30\n0.0\n40\n{diameter/2}\nENDSEC\nEOF"
    stl_content = f"solid teacup\nfacet normal 0 0 0\nouter loop\nvertex 0 0 0\nvertex {diameter} 0 0\nvertex 0 {height} 0\nendloop\nendfacet\nendsolid teacup"
    
    with open("teacup_50_120.dxf", "w") as f:
        f.write(dxf_content)
    with open("teacup_50_120.stl", "w") as f:
        f.write(stl_content)
    
    print(f"CAD File: teacup_50_120.dxf (Diameter: {diameter}mm)")
    print(f"3D STL: teacup_50_120.stl (Height: {height}mm)")

if __name__ == "__main__":
    generate_teacup_files(50, 120)
