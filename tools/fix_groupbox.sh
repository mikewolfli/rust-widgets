#!/bin/bash

# 修复 groupbox.rs 文件

echo "修复 groupbox.rs..."

# 修复 draw_rect 调用
sed -i 's/context\.draw_rect([[:space:]]*rect\.x,[[:space:]]*rect\.y,[[:space:]]*rect\.width,[[:space:]]*rect\.height,/context.draw_rect(rect,/g' src/widget/container_widgets/groupbox.rs

# 修复 fill_rect 调用
sed -i 's/context\.fill_rect([[:space:]]*title_bg_x,[[:space:]]*rect\.y,[[:space:]]*title_bg_width,[[:space:]]*2\.0,/context.fill_rect(Rect::new(title_bg_x as i32, rect.y, title_bg_width as u32, 2),/g' src/widget/container_widgets/groupbox.rs

# 修复 checkbox draw_rect 调用
sed -i 's/context\.draw_rect([[:space:]]*checkbox_rect\.x,[[:space:]]*checkbox_rect\.y,[[:space:]]*checkbox_rect\.width,[[:space:]]*checkbox_rect\.height,/context.draw_rect(checkbox_rect,/g' src/widget/container_widgets/groupbox.rs

# 修复 draw_line 调用
sed -i 's/context\.draw_line([[:space:]]*checkbox_rect\.x + 2\.0,[[:space:]]*checkbox_rect\.y + checkbox_rect\.height as i32 \/ 2,[[:space:]]*checkbox_rect\.x + checkbox_rect\.width as i32 \/ 2,[[:space:]]*checkbox_rect\.y + checkbox_rect\.height - 2\.0,/context.draw_line(Point::new(checkbox_rect.x + 2, checkbox_rect.y + checkbox_rect.height as i32 \/ 2), Point::new(checkbox_rect.x + checkbox_rect.width as i32 \/ 2, checkbox_rect.y + checkbox_rect.height - 2),/g' src/widget/container_widgets/groupbox.rs

sed -i 's/context\.draw_line([[:space:]]*checkbox_rect\.x + checkbox_rect\.width as i32 \/ 2,[[:space:]]*checkbox_rect\.y + checkbox_rect\.height - 2\.0,[[:space:]]*checkbox_rect\.x + checkbox_rect\.width - 2\.0,[[:space:]]*checkbox_rect\.y + 2\.0,/context.draw_line(Point::new(checkbox_rect.x + checkbox_rect.width as i32 \/ 2, checkbox_rect.y + checkbox_rect.height - 2), Point::new(checkbox_rect.x + checkbox_rect.width - 2, checkbox_rect.y + 2),/g' src/widget/container_widgets/groupbox.rs

# 修复 draw_text 调用
sed -i 's/context\.draw_text([[:space:]]*title_rect\.x,[[:space:]]*title_rect\.y,[[:space:]]*&self\.title,[[:space:]]*&Font::default(),[[:space:]]*text_color,/context.draw_text(Point::new(title_rect.x, title_rect.y), \&self.title, \&Font::default(),/g' src/widget/container_widgets/groupbox.rs

# 修复浮点数运算
sed -i 's/title_bg_width + 20\.0/title_bg_width + 20/g' src/widget/container_widgets/groupbox.rs
sed -i 's/title_rect\.x - 10\.0/title_rect.x - 10/g' src/widget/container_widgets/groupbox.rs
sed -i 's/checkbox_rect\.x + 2\.0/checkbox_rect.x + 2/g' src/widget/container_widgets/groupbox.rs
sed -i 's/checkbox_rect\.height - 2\.0/checkbox_rect.height - 2/g' src/widget/container_widgets/groupbox.rs
sed -i 's/checkbox_rect\.width - 2\.0/checkbox_rect.width - 2/g' src/widget/container_widgets/groupbox.rs
sed -i 's/checkbox_rect\.y + 2\.0/checkbox_rect.y + 2/g' src/widget/container_widgets/groupbox.rs

echo "groupbox.rs 修复完成"