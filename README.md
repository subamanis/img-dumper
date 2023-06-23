# img-dumper

This a CLI tool that searches for <b>image files</b> (svg, png, etc.) inside a root directory, and <b>generates an interactive .html file</b> that displays the icons,
their paths and groups them by project name. The generated file is interactible, searchable and contains extension filters.

Running the program with no arguments, tries to find the default installation path for <b>xampp/htdocs</b>. </br>
You can provide another path the *--dir* argument (see argument below)

Every top level folder inside the root directory, is considered a `project`.

## How To Run
The only thing you need is the <b>binary</b>. You can either:
1) For Windows only, grab the prebuilt binary from the "executable" folder.
2) ```cargo install --git https://github.com/subamanis/img-dumper```
3) Build yourself by cloning or downloading the repo (```cargo build --release```),

And to run it:
```img-dumper --optional_arg1 --optional_argN``` 


## Cmd Arguments
Below there is a list with all the arguments-flags that the program accepts.
```
--help
    No arguments or any number of existing other commands.

    Overrides normal program execution and just displays an informative message on the terminal.
    Ignores other arguments.

--dir
    1 argument, the path (relative or absolute) to the root directory.
    The path can contain whitespaces, no need to surround it with quatation marks.

    Every top level directory inside the root directory, is considered a project.

--target 
    1 argument, the path (relative or absolute) to the directory that the generated file will be saved.
    The path can contain whitespaces, no need to surround it with quatation marks.

--name 
    1 argument, the name of the generated file.
    The name can contain whitespaces.
