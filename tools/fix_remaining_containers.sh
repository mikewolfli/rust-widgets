#!/bin/bash

# 修复剩余的容器小部件文件

echo "修复剩余的容器小部件文件..."

# 修复 mdiarea.rs 中的 fill_rect 调用
sed -i 's/context\.fill_rect([[:space:]]*rect\.x,[[:space:]]*rect\.y,[[:space:]]*rect\.width,[[:space:]]*rect\.height,/context.fill_rect(rect,/g' src/widget/container_widgets/mdiarea.rs

# 修复 mdiarea.rs 中的 draw_rect 调用
sed -i 's/context\.draw_rect([[:space:]]*rect\.x,[[:space:]]*rect\.y,[[:space:]]*rect\.width,[[:space:]]*rect\.height,/context.draw_rect(rect,/g' src/widget/container_widgets/mdiarea.rs

# 修复 mdiarea.rs 中的 draw_line 调用
sed -i 's/context\.draw_line([[:space:]]*x1,[[:space:]]*y1,[[:space:]]*x2,[[:space:]]*y2,/context.draw_line(Point::new(x1, y1), Point::new(x2, y2),/g' src/widget/container_widgets/mdiarea.rs

# 修复 mdiarea.rs 中的 draw_text 调用
sed -i 's/context\.draw_text([[:space:]]*title_rect\.x + 5,[[:space:]]*title_rect\.y + title_rect\.height as i32 \/ 2,[[:space:]]*&subwindow\.title,[[:space:]]*&Font::default(),[[:space:]]*Color::from_rgb(0, 0, 0),/context.draw_text(Point::new(title_rect.x + 5, title_rect.y + title_rect.height as i32 \/ 2), \&subwindow.title, \&Font::default(),/g' src/widget/container_widgets/mdiarea.rs

# 修复其他容器小部件文件中的 fill_rect/draw_rect 调用
for file in src/widget/container_widgets/*.rs; do
    if [[ -f "$file" ]]; then
        # 修复 fill_rect 调用
        sed -i 's/context\.fill_rect([[:space:]]*\([a-zA-Z_][a-zA-Z0-9_]*\)\.x,[[:space:]]*\1\.y,[[:space:]]*\1\.width,[[:space:]]*\1\.height,/context.fill_rect(\1,/g' "$file"
        
        # 修复 draw_rect 调用
        sed -i 's/context\.draw_rect([[:space:]]*\([a-zA-Z_][a-zA-Z0-9_]*\)\.x,[[:space:]]*\1\.y,[[:space:]]*\1\.width,[[:space:]]*\1\.height,/context.draw_rect(\1,/g' "$file"
        
        # 修复 draw_line 调用（简单模式）
        sed -i 's/context\.draw_line([[:space:]]*\([a-zA-Z_][a-zA-Z0-9_]*\)\.x + \([0-9]\+\)\.0,[[:space:]]*\1\.y + \([0-9]\+\)\.0,[[:space:]]*\1\.x + \([0-9]\+\)\.0,[[:space:]]*\1\.y + \([0-9]\+\)\.0,/context.draw_line(Point::new(\1.x + \2, \1.y + \3), Point::new(\1.x + \4, \1.y + \5),/g' "$file"
    fi
done

echo "容器小部件文件修复完成"