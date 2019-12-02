use std::env;

use ossim_oxide::base::Model;
use ossim_oxide::model::nitf::NITF;

fn main() {
    let args: Vec<String> = env::args().collect();
    let filename = &args[1];

    let nitf = NITF::new(filename.to_string()).unwrap();
    println!("{}",nitf);
}
