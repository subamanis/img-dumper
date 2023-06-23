pub enum Argument {
    Dir,
    Target,
    Name,
    Basic,
    NoBrowser,
    Help,
}

impl Argument {
    pub fn get_name(&self) -> &str {
        match self {
            Argument::Dir         => "dir",
            Argument::Target      => "target",
            Argument::Name        => "name",
            Argument::Basic       => "basic",
            Argument::NoBrowser   => "no-browser",
            Argument::Help        => "help",
        }
    }

    pub fn get_help_msg(&self) -> &str {
        // weird formatting as a simple solution to keep the indentation when printing.
        match self {
Argument::Dir => "--dir
    1 argument, the path (relative or absolute) to the root directory.
    The path can contain whitespaces, no need to surround it with quatation marks.

    Every top level directory inside the root directory, is considered a project.
",
Argument::Target => "--target 
    1 argument, the path (relative or absolute) to the directory that the generated file will be saved.
    The path can contain whitespaces, no need to surround it with quatation marks.

",
Argument::Name => "--name 
    1 argument, the name of the generated file.
    The name can contain whitespaces.

",
Argument::Basic => "--basic 
    No arguments.
    Skips the parsing of special css files, that may not exist if the project is used for generic use.

",
Argument::NoBrowser => "--no-browser 
    No arguments.
    Doesn't try to open the geneated html in the browser.

",
_  => "",
    }
    }
}

pub fn print_whole_help_message() {
    let mut msg = 
    "Running the program without arguments will try to find the common default installation paths for xampp/htdocs
and consider this path as root.\n\n".to_owned();

    msg += Argument::Dir.get_help_msg();
    msg += Argument::Target.get_help_msg();
    msg += Argument::Name.get_help_msg();
    msg += Argument::Basic.get_help_msg();
    msg += Argument::NoBrowser.get_help_msg();

    print!("{}",msg);
}
