#!/bin/bash
# 修复draw_line调用：从5个参数改为3个参数（Point, Point, Color）
# 更精确的版本，处理多行调用

echo "修复draw_line调用（版本2）..."

# 处理frame.rs文件
echo "处理frame.rs..."
sed -i -E '
# 匹配5个参数的draw_line调用
/context\.draw_line\(/,/\);/ {
    # 如果是draw_line行
    /context\.draw_line\(/ {
        N;N;N;N;N  # 读取接下来的5行
        /context\.draw_line\([^)]*,[^)]*,[^)]*,[^)]*,[^)]*\)/ {
            # 提取5个参数
            s/context\.draw_line\(([^,]+),\s*([^,]+),\s*([^,]+),\s*([^,]+),\s*([^)]+)\)/context.draw_line(Point::new(\1, \2), Point::new(\3, \4), \5)/
        }
    }
}' src/widget/base_widgets/frame.rs

echo "修复完成"