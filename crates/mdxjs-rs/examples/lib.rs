extern crate mdxjs;

/// Example that compiles the example MDX document from <https://mdxjs.com>
/// to JavaScript.
fn main() -> Result<(), tryckeri_mdast::mdx_types::Message> {
    println!(
        "{}",
        mdxjs::compile(
            r##"
import {Chart} from './snowfall.js'
export const year = 2018

# Last year’s snowfall

In {year}, the snowfall was above average.
It was followed by a warm spring which caused
flood conditions in many of the nearby rivers.

<Chart year={year} color="#fcb32c" />
"##,
            &Default::default()
        )?
    );

    Ok(())
}
