#!/bin/bash
# 修复draw_text调用

echo "修复draw_text调用..."

# 处理draw_text调用：移除对齐参数
find src/widget -name "*.rs" -type f | while read file; do
    echo "检查文件: $file"
    
    # 检查是否有draw_text调用
    if grep -q "draw_text(" "$file"; then
        echo "  -> 发现draw_text调用"
        
        # 创建备份
        cp "$file" "$file.bak"
        
        # 修复draw_text调用：移除对齐参数
        # 旧API: draw_text(x, y, text, alignment, color)
        # 新API: draw_text(rect, text, color) 或 draw_text(point, text, color)
        sed -i -E '
            # 处理draw_text(x, y, text, alignment, color) -> draw_text(Point::new(x, y), text, color)
            s/draw_text\(([^,]+),\s*([^,]+),\s*([^,]+),\s*([^,]+),\s*([^)]+)\)/draw_text(Point::new(\1, \2), \3, \5)/g
            
            # 处理draw_text(rect, text, alignment, color) -> draw_text(rect, text, color)
            s/draw_text\(([^,]+),\s*([^,]+),\s*([^,]+),\s*([^)]+)\)/draw_text(\1, \2, \4)/g
        ' "$file"
        
        # 检查是否有变化
        if diff -q "$file" "$file.bak" > /dev/null; then
            rm "$file.bak"
            echo "  -> 无变化"
        else
            echo "  -> 已修复"
            rm "$file.bak"
        fi
    fi
done

echo "修复完成"