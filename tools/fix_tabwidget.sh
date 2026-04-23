#!/bin/bash

# 修复 tabwidget.rs 文件

echo "修复 tabwidget.rs..."

# 修复 content_rect 的 fill_rect 调用
sed -i 's/context\.fill_rect([[:space:]]*content_rect\.x,[[:space:]]*content_rect\.y,[[:space:]]*content_rect\.width,[[:space:]]*content_rect\.height,/context.fill_rect(content_rect,/g' src/widget/container_widgets/tabwidget.rs

# 修复 content_rect 的 draw_rect 调用
sed -i 's/context\.draw_rect([[:space:]]*content_rect\.x,[[:space:]]*content_rect\.y,[[:space:]]*content_rect\.width,[[:space:]]*content_rect\.height,/context.draw_rect(content_rect,/g' src/widget/container_widgets/tabwidget.rs

# 修复 tab_rect 的 fill_rect 调用
sed -i 's/context\.fill_rect([[:space:]]*tab_rect\.x,[[:space:]]*tab_rect\.y,[[:space:]]*tab_rect\.width,[[:space:]]*tab_rect\.height,/context.fill_rect(tab_rect,/g' src/widget/container_widgets/tabwidget.rs

# 修复 tab_rect 的 draw_rect 调用
sed -i 's/context\.draw_rect([[:space:]]*tab_rect\.x,[[:space:]]*tab_rect\.y,[[:space:]]*tab_rect\.width,[[:space:]]*tab_rect\.height,/context.draw_rect(tab_rect,/g' src/widget/container_widgets/tabwidget.rs

# 修复 draw_text 调用
sed -i 's/context\.draw_text([[:space:]]*tab_rect\.x + tab_rect\.width as i32 \/ 2,[[:space:]]*tab_rect\.y + tab_rect\.height as i32 \/ 2,[[:space:]]*&tab\.title,[[:space:]]*&Font::default(),[[:space:]]*text_color,/context.draw_text(Point::new(tab_rect.x + tab_rect.width as i32 \/ 2, tab_rect.y + tab_rect.height as i32 \/ 2), \&tab.title, \&Font::default(),/g' src/widget/container_widgets/tabwidget.rs

# 修复 draw_line 调用
sed -i 's/context\.draw_line([[:space:]]*close_x,[[:space:]]*close_y,[[:space:]]*close_x + close_size,[[:space:]]*close_y + close_size,/context.draw_line(Point::new(close_x as i32, close_y as i32), Point::new((close_x + close_size) as i32, (close_y + close_size) as i32),/g' src/widget/container_widgets/tabwidget.rs

sed -i 's/context\.draw_line([[:space:]]*close_x + close_size,[[:space:]]*close_y,[[:space:]]*close_x,[[:space:]]*close_y + close_size,/context.draw_line(Point::new((close_x + close_size) as i32, close_y as i32), Point::new(close_x as i32, (close_y + close_size) as i32),/g' src/widget/container_widgets/tabwidget.rs

# 修复浮点数运算
sed -i 's/close_size = 12\.0/close_size = 12/g' src/widget/container_widgets/tabwidget.rs
sed -i 's/close_x = tab_rect\.x + tab_rect\.width - close_size - 5\.0/close_x = tab_rect.x + tab_rect.width - close_size - 5/g' src/widget/container_widgets/tabwidget.rs
sed -i 's/close_y = tab_rect\.y + (tab_rect\.height - close_size) \/ 2\.0/close_y = tab_rect.y + (tab_rect.height - close_size) \/ 2/g' src/widget/container_widgets/tabwidget.rs

echo "tabwidget.rs 修复完成"