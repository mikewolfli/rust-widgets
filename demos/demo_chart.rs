//! Chart demo

use rust_widgets::core::{Rect, Color};
use rust_widgets::chart::{Chart, ChartSeries, DataPoint, LineChart, MemoryChartContext};
use rust_widgets::widget::{Widget, Window};
use rust_widgets::{init, run};

fn main() {
    // Initialize the GUI library
    init();
    
    // Create a window
    let mut window = Window::new(
        "Chart Demo".to_string(),
        Rect { x: 100, y: 100, width: 800, height: 600 }
    );
    
    let mut chart = LineChart::new();
    chart.set_title("Sales Data".to_string());
    chart.set_x_axis_label("Month".to_string());
    chart.set_y_axis_label("Sales".to_string());
    
    // Create sample data
    let mut data = Vec::new();
    data.push(DataPoint { x: 1.0, y: 100.0, label: Some("Jan".to_string()) });
    data.push(DataPoint { x: 2.0, y: 150.0, label: Some("Feb".to_string()) });
    data.push(DataPoint { x: 3.0, y: 120.0, label: Some("Mar".to_string()) });
    data.push(DataPoint { x: 4.0, y: 200.0, label: Some("Apr".to_string()) });
    data.push(DataPoint { x: 5.0, y: 180.0, label: Some("May".to_string()) });
    
    // Create a series
    let series = ChartSeries {
        name: "2024 Sales".to_string(),
        data,
        color: Color { r: 33, g: 150, b: 243, a: 255 },
        visible: true,
    };
    
    // Add the series to the chart
    chart.add_series(series);
    
    let mut context = MemoryChartContext::default();
    chart.draw(Rect { x: 16, y: 16, width: 640, height: 320 }, &mut context);
    println!("draw commands: {}", context.commands.len());
    
    window.show();

    run();
}
