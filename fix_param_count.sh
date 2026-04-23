#!/bin/bash
# 修复参数数量错误

echo "修复参数数量错误..."

# 修复draw_rect调用：从3个参数改为2个参数
find src/widget -name "*.rs" -type f | while read file; do
    echo "检查文件: $file"
    
    # 检查是否有draw_rect(rect, color, width)调用
    if grep -q "draw_rect([^)]*,[^)]*,[^)]*)" "$file"; then
        echo "  -> 发现draw_rect调用"
        
        # 创建备份
        cp "$file" "$file.bak"
        
        # 修复draw_rect调用：移除width参数
        sed -i -E '
            # 处理draw_rect(rect, color, width) -> draw_rect(rect, color)
            s/draw_rect\(([^,]+),\s*([^,]+),\s*([^)]+)\)/draw_rect(\1, \2)/g
        ' "$file"
        
        # 检查是否有变化
        if diff -q "$file" "$file.bak" > /dev/null; then
            rm "$file.bak"
            echo "  -> 无变化"
        else
            echo "  -> 已修复draw_rect调用"
            rm "$file.bak"
        fi
    fi
    
    # 检查是否有draw_circle或fill_circle调用
    if grep -q "draw_circle\|fill_circle" "$file"; then
        echo "  -> 发现circle调用"
        
        if [ ! -f "$file.bak" ]; then
            cp "$file" "$file.bak"
        fi
        
        # 修复circle调用：确保有3个参数
        sed -i -E '
            # 处理draw_circle(Point::new(center, radius), color) -> draw_circle(center, radius, color)
            s/draw_circle\(Point::new\(([^,]+),\s*([^)]+)\),\s*([^)]+)\)/draw_circle(\1, \2, \3)/g
            s/fill_circle\(Point::new\(([^,]+),\s*([^)]+)\),\s*([^)]+)\)/fill_circle(\1, \2, \3)/g
        ' "$file"
        
        echo "  -> 已修复circle调用"
    fi
    
    # 清理备份
    [ -f "$file.bak" ] && rm "$file.bak"
done

echo "修复完成"