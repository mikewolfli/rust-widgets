#!/bin/bash
# 修复类型不匹配问题

echo "修复类型不匹配问题..."

# 修复所有文件中的类型转换问题
find src -name "*.rs" -type f | while read file; do
    echo "检查 $file"
    cp "$file" "$file.bak"
    
    # 修复 f32 和 i32 的混合运算
    sed -i -E '
        # 修复 rect.x + line_width as i32 中的类型问题
        s/rect\.x \+ line_width as i32/rect.x + line_width as f32/g
        s/rect\.y \+ line_width as i32/rect.y + line_width as f32/g
        s/rect\.x \+ rect\.width as i32/rect.x + rect.width as f32/g
        s/rect\.y \+ rect\.height as i32/rect.y + rect.height as f32/g
        
        # 修复减法中的类型问题
        s/rect\.width as i32 - line_width as i32/rect.width as f32 - line_width as f32/g
        s/rect\.height as i32 - line_width as i32/rect.height as f32 - line_width as f32/g
        
        # 修复 Point::new 中的类型转换
        s/Point::new\(([^,]+), ([^)]+)\)/Point::new(\1 as f32, \2 as f32)/g
    ' "$file"
    
    if diff -q "$file" "$file.bak" > /dev/null; then
        echo "  $file: 无变化"
    else
        echo "  $file: 已修复"
    fi
    rm "$file.bak"
done

echo "修复完成"