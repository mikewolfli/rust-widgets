#!/bin/bash
# 简单修复draw_line调用：只处理明显的5参数调用

echo "简单修复draw_line调用..."

# 查找所有包含5参数draw_line调用的文件
find src -name "*.rs" -type f -exec grep -l "draw_line([^)]*,[^)]*,[^)]*,[^)]*,[^)]*)" {} \; | while read file; do
    echo "处理文件: $file"
    
    # 创建备份
    cp "$file" "$file.bak"
    
    # 使用sed修复简单的5参数调用
    sed -i -E '
        # 处理单行调用
        s/draw_line\(([^,]+),\s*([^,]+),\s*([^,]+),\s*([^,]+),\s*([^)]+)\)/draw_line(Point::new(\1, \2), Point::new(\3, \4), \5)/g
    ' "$file"
    
    # 检查是否有变化
    if diff -q "$file" "$file.bak" > /dev/null; then
        rm "$file.bak"
    else
        echo "  -> 已修复"
    fi
done

echo "简单修复完成"