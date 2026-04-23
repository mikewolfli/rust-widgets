#!/bin/bash

# 修复 scrollarea.rs 文件

echo "修复 scrollarea.rs..."

# 修复 rect 的 fill_rect 调用
sed -i 's/context\.fill_rect([[:space:]]*rect\.x,[[:space:]]*rect\.y,[[:space:]]*rect\.width,[[:space:]]*rect\.height,/context.fill_rect(rect,/g' src/widget/container_widgets/scrollarea.rs

# 修复 rect 的 draw_rect 调用
sed -i 's/context\.draw_rect([[:space:]]*rect\.x,[[:space:]]*rect\.y,[[:space:]]*rect\.width,[[:space:]]*rect\.height,/context.draw_rect(rect,/g' src/widget/container_widgets/scrollarea.rs

# 修复水平滚动条的 fill_rect 调用
sed -i 's/context\.fill_rect([[:space:]]*rect\.x,[[:space:]]*scroll_bar_y,[[:space:]]*rect\.width,[[:space:]]*scroll_bar_height,/context.fill_rect(Rect::new(rect.x, scroll_bar_y, rect.width, scroll_bar_height as u32),/g' src/widget/container_widgets/scrollarea.rs

# 修复水平滚动条的 draw_rect 调用
sed -i 's/context\.draw_rect([[:space:]]*rect\.x,[[:space:]]*scroll_bar_y,[[:space:]]*rect\.width,[[:space:]]*scroll_bar_height,/context.draw_rect(Rect::new(rect.x, scroll_bar_y, rect.width, scroll_bar_height as u32),/g' src/widget/container_widgets/scrollarea.rs

# 修复水平滚动条滑块的 fill_rect 调用
sed -i 's/context\.fill_rect([[:space:]]*thumb_x,[[:space:]]*scroll_bar_y,[[:space:]]*thumb_width,[[:space:]]*scroll_bar_height,/context.fill_rect(Rect::new(thumb_x as i32, scroll_bar_y, thumb_width as u32, scroll_bar_height as u32),/g' src/widget/container_widgets/scrollarea.rs

# 修复垂直滚动条的 fill_rect 调用
sed -i 's/context\.fill_rect([[:space:]]*scroll_bar_x,[[:space:]]*rect\.y,[[:space:]]*scroll_bar_width,[[:space:]]*rect\.height,/context.fill_rect(Rect::new(scroll_bar_x, rect.y, scroll_bar_width as u32, rect.height),/g' src/widget/container_widgets/scrollarea.rs

# 修复垂直滚动条的 draw_rect 调用
sed -i 's/context\.draw_rect([[:space:]]*scroll_bar_x,[[:space:]]*rect\.y,[[:space:]]*scroll_bar_width,[[:space:]]*rect\.height,/context.draw_rect(Rect::new(scroll_bar_x, rect.y, scroll_bar_width as u32, rect.height),/g' src/widget/container_widgets/scrollarea.rs

# 修复垂直滚动条滑块的 fill_rect 调用
sed -i 's/context\.fill_rect([[:space:]]*scroll_bar_x,[[:space:]]*thumb_y,[[:space:]]*scroll_bar_width,[[:space:]]*thumb_height,/context.fill_rect(Rect::new(scroll_bar_x, thumb_y as i32, scroll_bar_width as u32, thumb_height as u32),/g' src/widget/container_widgets/scrollarea.rs

# 修复角落的 fill_rect 调用
sed -i 's/context\.fill_rect([[:space:]]*corner_x,[[:space:]]*corner_y,[[:space:]]*corner_size,[[:space:]]*corner_size,/context.fill_rect(Rect::new(corner_x, corner_y, corner_size as u32, corner_size as u32),/g' src/widget/container_widgets/scrollarea.rs

# 修复浮点数运算
sed -i 's/scroll_bar_height = 16\.0/scroll_bar_height = 16/g' src/widget/container_widgets/scrollarea.rs
sed -i 's/scroll_bar_width = 16\.0/scroll_bar_width = 16/g' src/widget/container_widgets/scrollarea.rs
sed -i 's/corner_size = 16\.0/corner_size = 16/g' src/widget/container_widgets/scrollarea.rs
sed -i 's/thumb_width = rect\.width \* 0\.3/thumb_width = rect.width \* 3 \/ 10/g' src/widget/container_widgets/scrollarea.rs
sed -i 's/thumb_height = rect\.height \* 0\.3/thumb_height = rect.height \* 3 \/ 10/g' src/widget/container_widgets/scrollarea.rs

echo "scrollarea.rs 修复完成"