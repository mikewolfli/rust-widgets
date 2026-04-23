#!/bin/bash
# 最终修复frame.rs文件

echo "最终修复frame.rs文件..."

file="src/widget/base_widgets/frame.rs"
cp "$file" "$file.bak"

# 修复所有类型转换问题
sed -i -E '
    # 修复 rect.x + rect.width as f32 -> rect.x as f32 + rect.width as f32
    s/rect\.x \+ rect\.width as f32/rect.x as f32 + rect.width as f32/g
    s/rect\.y \+ rect\.height as f32/rect.y as f32 + rect.height as f32/g
    
    # 修复 rect.x + line_width as f32 -> rect.x as f32 + line_width as f32
    s/rect\.x \+ line_width as f32/rect.x as f32 + line_width as f32/g
    s/rect\.y \+ line_width as f32/rect.y as f32 + line_width as f32/g
    
    # 修复 rect.width as f32 - line_width as f32 -> rect.width as f32 - line_width as f32
    s/rect\.width as f32 - line_width as f32/rect.width as f32 - line_width as f32/g
    s/rect\.height as f32 - line_width as f32/rect.height as f32 - line_width as f32/g
    
    # 修复 Point::new(rect.x, rect.y) -> Point::new(rect.x as f32, rect.y as f32)
    s/Point::new\(rect\.x, rect\.y\)/Point::new(rect.x as f32, rect.y as f32)/g
' "$file"

# 检查修复结果
if diff -q "$file" "$file.bak" > /dev/null; then
    echo "无变化"
else
    echo "已修复frame.rs"
    diff -u "$file.bak" "$file" | head -50
fi

rm "$file.bak"
echo "修复完成"