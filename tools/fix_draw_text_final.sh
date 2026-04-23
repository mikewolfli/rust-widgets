#!/bin/bash
# 修复draw_text调用 - 最终版本

echo "修复draw_text调用..."

# 处理draw_text调用：移除font和alignment参数
find src/widget -name "*.rs" -type f | while read file; do
    echo "检查文件: $file"
    
    # 检查是否有draw_text调用
    if grep -q "draw_text(" "$file"; then
        echo "  -> 发现draw_text调用"
        
        # 创建备份
        cp "$file" "$file.bak"
        
        # 修复模式1: draw_text(x, y, text, font, color, alignment) -> draw_text(Point::new(x, y), text, color)
        sed -i -E '
            # 处理6参数调用
            s/draw_text\(([^,]+),\s*([^,]+),\s*([^,]+),\s*([^,]+),\s*([^,]+),\s*([^)]+)\)/draw_text(Point::new(\1, \2), \3, \5)/g
            
            # 处理5参数调用: draw_text(x, y, text, font, color) -> draw_text(Point::new(x, y), text, color)
            s/draw_text\(([^,]+),\s*([^,]+),\s*([^,]+),\s*([^,]+),\s*([^)]+)\)/draw_text(Point::new(\1, \2), \3, \5)/g
            
            # 处理4参数调用: draw_text(rect, text, color, alignment) -> draw_text(rect, text, color)
            s/draw_text\(([^,]+),\s*([^,]+),\s*([^,]+),\s*([^)]+)\)/draw_text(\1, \2, \3)/g
        ' "$file"
        
        # 检查是否有变化
        if diff -q "$file" "$file.bak" > /dev/null; then
            rm "$file.bak"
            echo "  -> 无变化"
        else
            echo "  -> 已修复"
            # 显示修复示例
            diff -u "$file.bak" "$file" | grep -E "^[-+].*draw_text" | head -5
            rm "$file.bak"
        fi
    fi
done

echo "修复完成"