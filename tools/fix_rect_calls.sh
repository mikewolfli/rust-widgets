#!/bin/bash
# 修复fill_rect和draw_rect调用

echo "修复fill_rect和draw_rect调用..."

# 处理fill_rect调用：从5个参数改为2个参数（Rect, Color）
find src -name "*.rs" -type f | while read file; do
    echo "检查文件: $file"
    
    # 检查是否有fill_rect调用
    if grep -q "fill_rect([^)]*,[^)]*,[^)]*,[^)]*,[^)]*)" "$file"; then
        echo "  -> 处理fill_rect调用"
        
        # 创建备份
        cp "$file" "$file.bak"
        
        # 修复fill_rect调用
        sed -i -E '
            # 处理fill_rect(x, y, width, height, color) -> fill_rect(Rect::new(x, y, width, height), color)
            s/fill_rect\(([^,]+),\s*([^,]+),\s*([^,]+),\s*([^,]+),\s*([^)]+)\)/fill_rect(Rect::new(\1, \2, \3, \4), \5)/g
        ' "$file"
        
        # 检查是否有变化
        if diff -q "$file" "$file.bak" > /dev/null; then
            rm "$file.bak"
        else
            echo "  -> 已修复fill_rect调用"
        fi
    fi
    
    # 检查是否有draw_rect调用
    if grep -q "draw_rect([^)]*,[^)]*,[^)]*,[^)]*,[^)]*)" "$file"; then
        echo "  -> 处理draw_rect调用"
        
        if [ ! -f "$file.bak" ]; then
            cp "$file" "$file.bak"
        fi
        
        # 修复draw_rect调用
        sed -i -E '
            # 处理draw_rect(x, y, width, height, color) -> draw_rect(Rect::new(x, y, width, height), color)
            s/draw_rect\(([^,]+),\s*([^,]+),\s*([^,]+),\s*([^,]+),\s*([^)]+)\)/draw_rect(Rect::new(\1, \2, \3, \4), \5)/g
        ' "$file"
        
        echo "  -> 已修复draw_rect调用"
    fi
    
    # 清理备份
    [ -f "$file.bak" ] && rm "$file.bak"
done

echo "修复完成"