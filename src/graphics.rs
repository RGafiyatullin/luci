pub enum OutputFormat {
    Graphviz,
}

pub trait RenderGraph {
    const OUTPUT: OutputFormat;

    fn render(&self) -> String;
}
