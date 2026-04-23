#!/bin/bash
# 修复draw_line调用：从5个参数改为3个参数（Point, Point, Color）

echo "修复draw_line调用..."

# 查找所有draw_line调用并修复
find src -name "*.rs" -type f | while read file; do
    echo "处理文件: $file"
    
    # 临时文件
    tmp_file="${file}.tmp"
    
    # 修复模式1: draw_line(x1, y1, x2, y2, color) -> draw_line(Point::new(x1, y1), Point::new(x2, y2), color)
    sed -E 's/context\.draw_line\(([^,]+),\s*([^,]+),\s*([^,]+),\s*([^,]+),\s*([^)]+)\)/context.draw_line(Point::new(\1, \2), Point::new(\3, \4), \5)/g' "$file" > "$tmp_file"
    
    # 如果文件有变化，替换原文件
    if ! cmp -s "$file" "$tmp_file"; then
        mv "$tmp_file" "$file"
        echo "  -> 已修复draw_line调用"
    else
        rm "$tmp_file"
    fi
done

echo "draw_line修复完成"