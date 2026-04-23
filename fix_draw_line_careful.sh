#!/bin/bash
# 谨慎修复draw_line调用

echo "谨慎修复draw_line调用..."

# 只处理widget目录下的文件
find src/widget -name "*.rs" -type f | while read file; do
    echo "检查文件: $file"
    
    # 检查是否有5参数的draw_line调用
    if grep -q "draw_line([^)]*,[^)]*,[^)]*,[^)]*,[^)]*)" "$file"; then
        echo "  -> 发现5参数draw_line调用"
        
        # 创建备份
        cp "$file" "$file.bak"
        
        # 使用更精确的sed模式
        sed -i -E '
            # 处理简单的单行调用
            s/context\.draw_line\(([^,]+),\s*([^,]+),\s*([^,]+),\s*([^,]+),\s*([^)]+)\)/context.draw_line(Point::new(\1, \2), Point::new(\3, \4), \5)/g
            
            # 处理带类型转换的调用
            s/context\.draw_line\(([^,]+),\s*([^,]+),\s*([^,]+),\s*([^,]+)\s*-\s*line_width,\s*([^)]+)\)/context.draw_line(Point::new(\1, \2), Point::new(\3 - line_width as i32, \4), \5)/g
            s/context\.draw_line\(([^,]+),\s*([^,]+),\s*([^,]+)\s*-\s*line_width,\s*([^,]+),\s*([^)]+)\)/context.draw_line(Point::new(\1, \2), Point::new(\3 - line_width as i32, \4), \5)/g
        ' "$file"
        
        # 检查是否有变化
        if diff -q "$file" "$file.bak" > /dev/null; then
            rm "$file.bak"
            echo "  -> 无变化"
        else
            echo "  -> 已修复"
            # 显示修复的内容
            diff -u "$file.bak" "$file" | head -20
            rm "$file.bak"
        fi
    fi
done

echo "修复完成"