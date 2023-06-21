
pub const DIR      :&str   = "dir";
pub const TARGET   :&str   = "target";
pub const NAME     :&str   = "name";
pub const HELP     :&str   = "help";

const DIR_HELP  :  &str = 
"--dir
    1 argument, the path (relative or absolute) to the root directory.
    The path can contain whitespaces, no need to surround it with quatation marks.

    Every top level directory inside the root directory, is considered a project.

"; 
const TARGET_HELP  :  &str = 
"--target 
    1 argument, the path (relative or absolute) to the directory that the generated file will be saved.
    The path can contain whitespaces, no need to surround it with quatation marks.

"; 
const NAME_HELP  :  &str = 
"--name 
    1 argument, the name of the generated file.
    The name can contain whitespaces.

"; 


pub fn print_whole_help_message() {
    let mut msg = String::new();

    msg += DIR_HELP;
    msg += TARGET_HELP;
    msg += NAME_HELP;


    println!("{}",msg);
}

pub fn print_help_message_for_command(arg: &str) {
    if let Some(x) = get_help_msg_of_command(arg) {
        println!("\n{}",x);
    } 
}

fn get_help_msg_of_command(command: &str) -> Option<&str> {
    match command {
        DIR => Some(DIR_HELP),
        TARGET => Some(TARGET_HELP),
        NAME => Some(NAME_HELP),
        _ => None
    }
}
