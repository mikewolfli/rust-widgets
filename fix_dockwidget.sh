#!/bin/bash

# 修复 dockwidget.rs 文件

echo "修复 dockwidget.rs..."

# 修复 fill_rect 调用
sed -i 's/context\.fill_rect([[:space:]]*title_bar\.x,[[:space:]]*title_bar\.y,[[:space:]]*title_bar\.width,[[:space:]]*title_bar\.height,/context.fill_rect(title_bar,/g' src/widget/container_widgets/dockwidget.rs

# 修复 draw_rect 调用  
sed -i 's/context\.draw_rect([[:space:]]*title_bar\.x,[[:space:]]*title_bar\.y,[[:space:]]*title_bar\.width,[[:space:]]*title_bar\.height,/context.draw_rect(title_bar,/g' src/widget/container_widgets/dockwidget.rs

# 修复 draw_text 调用 - 移除对齐参数
sed -i 's/context\.draw_text([[:space:]]*title_bar\.x + 5\.0,[[:space:]]*title_bar\.y + title_bar\.height \/ 2\.0,[[:space:]]*&self\.title,[[:space:]]*&Font::default(),[[:space:]]*Color::from_rgb(0, 0, 0),/context.draw_text(Point::new(title_bar.x + 5, title_bar.y + title_bar.height as i32 \/ 2), \&self.title, \&Font::default(),/g' src/widget/container_widgets/dockwidget.rs

# 修复其他 fill_rect 调用
sed -i 's/context\.fill_rect([[:space:]]*close_rect\.x,[[:space:]]*close_rect\.y,[[:space:]]*close_rect\.width,[[:space:]]*close_rect\.height,/context.fill_rect(close_rect,/g' src/widget/container_widgets/dockwidget.rs

# 修复其他 draw_rect 调用
sed -i 's/context\.draw_rect([[:space:]]*close_rect\.x,[[:space:]]*close_rect\.y,[[:space:]]*close_rect\.width,[[:space:]]*close_rect\.height,/context.draw_rect(close_rect,/g' src/widget/container_widgets/dockwidget.rs

# 修复浮点数运算
sed -i 's/title_bar\.x + 5\.0/title_bar.x + 5/g' src/widget/container_widgets/dockwidget.rs
sed -i 's/title_bar\.height \/ 2\.0/title_bar.height as i32 \/ 2/g' src/widget/container_widgets/dockwidget.rs

echo "dockwidget.rs 修复完成"