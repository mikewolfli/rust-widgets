#!/bin/bash
# 精确修复frame.rs文件

echo "精确修复frame.rs文件..."

file="src/widget/base_widgets/frame.rs"
cp "$file" "$file.bak"

# 修复重复的as f32
sed -i -E '
    s/as f32 as f32/as f32/g
    s/as i32 as i32/as i32/g
' "$file"

# 修复Point::new中的类型转换
sed -i -E '
    # 修复 Point::new(rect.x as f32, rect.y as f32)
    s/Point::new\(rect\.x as f32, rect\.y as f32\)/Point::new(rect.x, rect.y)/g
    s/Point::new\(rect\.x \+ rect\.width as f32, rect\.y as f32\)/Point::new(rect.x + rect.width as f32, rect.y)/g
    s/Point::new\(rect\.x as f32, rect\.y \+ rect\.height as f32\)/Point::new(rect.x, rect.y + rect.height as f32)/g
    s/Point::new\(rect\.x \+ rect\.width as f32, rect\.y \+ rect\.height as f32\)/Point::new(rect.x + rect.width as f32, rect.y + rect.height as f32)/g
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