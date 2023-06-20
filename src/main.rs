use std::{fs::{File}, io::{Write, BufReader, BufRead}, collections::HashMap, process::Command, path::{PathBuf, Path}, env, time::Instant};

use anyhow::{Context, anyhow};
use colored::*;
use image::GenericImageView;
use walkdir::WalkDir;

fn main() -> anyhow::Result<()> {
    let instant = Instant::now();

    // Only on windows, it is required to enable a virtual terminal environment, so that the colors will display correctly
    #[cfg(target_os = "windows")]
    control::set_virtual_terminal(true).unwrap();

    let mut app_config = AppConfig::init()?;
    println!("Root folder: {}\n", app_config.root_folder);

    print!("Parsing projects... ");
    let mut projects_map = traverse_root_dir_and_make_project_map(&app_config);
    projects_map.retain(|_, project_dir| !project_dir.images.is_empty());
    if projects_map.is_empty() {
        println!("{}", "No icons could be found for any projects".yellow());
    } else {
        println!("{} ({} found)", "OK".green(), projects_map.len());
    }
    let mut sorted_project_names: Vec<String> = projects_map.keys().into_iter().map(|k| k.clone()).collect();
    sorted_project_names.sort();

    print!("Parsing sp-icons... ");
    let (sp_icons_class_names, sp_icons_css_string) =
            match parse_special_file(SpecialFileType::SpIconsCss, &projects_map, &mut app_config)? {
        Some((sp_icons_class_names, sp_icons_css_string)) => {
            println!("{}", "OK".green());
            (sp_icons_class_names, sp_icons_css_string)
        },
        None => {
            println!("{}", "No sp-icons file found".yellow());
            (Vec::new(), String::new())
        }
    };
    print!("Parsing font-awesome... ");
    let (font_awesome_class_names, font_awesome_css_string) =
            match parse_special_file(SpecialFileType::FontAwesomeCss, &projects_map, &mut app_config)? {
        Some((font_awesome_class_names, font_awesome_css_string)) => {
            println!("{}", "OK".green());
            (font_awesome_class_names, font_awesome_css_string)
        },
        None => {
            println!("{}", "No font-awesome file found".yellow());
            (Vec::new(), String::new())
        }
    };

    let html = generate_html_page_as_string(&projects_map, &sorted_project_names, &sp_icons_class_names, &sp_icons_css_string,
        &font_awesome_class_names, &font_awesome_css_string, &app_config);
    write_to_file(html, &app_config)?;
    println!("\nGenerated html file: {}", app_config.output_file_path);

    open_generated_file_in_the_browser(&app_config);

    println!("\nExec time: {:.2} secs", instant.elapsed().as_secs_f32());

    Ok(())
}

fn traverse_root_dir_and_make_project_map(app_config: &AppConfig) -> HashMap<String, ProjectDir> {
    let mut project_dirs = HashMap::new();

    let mut images = vec![];
    let mut project_dir = &mut ProjectDir::default();

    for entry in WalkDir::new(&app_config.root_folder)
            .follow_links(true)
            .into_iter()
            .filter_entry(|e| {
                e.file_name()
                .to_str()
                .map(|s| 
                    !s.starts_with(".") &&
                    !app_config.irrelevant_dir_names.contains(&s) &&
                    (e.depth() != 1 || e.file_type().is_dir()))
                .unwrap_or(false)
    }) {
        let entry = if let Ok(x) = entry { x } else { continue };
        let entry_path = entry.path();
        let entry_name = entry_path
            .file_name()
            .and_then(|os_str| os_str.to_str())
            .unwrap_or("");

        // if it is top-level project folder
        if entry.depth() == 1 {
            project_dir.images = images;

            project_dir = project_dirs
                .entry(entry_name.to_owned())
                .or_insert_with(|| ProjectDir {
                    name: entry_name.to_string(),
                    path: entry_path.to_string_lossy().into_owned(),
                    images: Vec::new(),
                });

            images = vec![];
        }

        let extension = entry_path.extension().unwrap_or_default().to_str().unwrap_or_default();

        if app_config.relevant_extensions.contains(&extension) {
            if extension == "png" {
                // check if the aspect ratio is bigger than 0.7 or smaller than 1.3
                let img = image::open(entry_path).unwrap();
                let (width, height) = img.dimensions();
                let aspect_ratio = width as f32 / height as f32;
                if aspect_ratio < 0.75 || aspect_ratio > 1.25 {
                    continue;
                }
            }
            let name = entry_path.file_stem().unwrap_or_default().to_str().unwrap_or_default();
            let img = Img {
                name: name.to_owned(),
                path: entry_path.to_str().unwrap_or_default().to_owned(),
                extension: extension.to_owned(),
            };
            images.push(img);
        }
    }

    project_dir.images = images;

    project_dirs
}

fn get_javascript_string(projects_names: &Vec<String>) -> String {
    let mut all_names = projects_names.clone();
    all_names.append(vec!["sp-icons".to_string(), "fa".to_string()].as_mut());
    let mut js_html = "<script>
    const inputElement = document.getElementById('search-input');
    inputElement.addEventListener('input', handleSearchChange);

    function handleSearchChange() {{
        var input, filter, ul, li, a, i, txtValue;
        input = document.getElementById('search-input');
        filter = input.value.toUpperCase();
        let relevant_lis_count = 0;".to_owned();
    for name in all_names {
        js_html.push_str(&format!("
        ul = document.getElementById('{}-icons-list');
        parent_project_area_div = ul.parentElement;
        li = ul.getElementsByTagName('li');
        for (i = 0; i < li.length; i++) {{
            let span = li[i].getElementsByTagName('span')[1];
            txtValue = span.textContent || span.innerText;
            if (txtValue.toUpperCase().indexOf(filter) > -1) {{
                li[i].style.display = '';
                relevant_lis_count += 1;
            }} else {{
                li[i].style.display = 'none';
            }}
        }}
        if (relevant_lis_count == 0) {{
            parent_project_area_div.style.display = 'none';
        }} else {{
            parent_project_area_div.style.display = 'block';
        }}
        relevant_lis_count = 0;
        ", name));
    }
    js_html.push_str("}} </script>");

    js_html
}

fn generate_html_page_as_string(
        project_dirs: &HashMap<String, ProjectDir>,
        sorted_project_names: &Vec<String>,
        sp_icons_class_names: &Vec<String>,
        sp_icons_css_string: &String,
        font_awesome_class_names: &Vec<String>,
        font_awesome_css_string: &String,
        app_config: &AppConfig) -> String {
    let mut html = String::from("<html> <head> <title>Spectre icons</title> </head> <body>");
    
    html += "<div class='search-container'>
                <div>
                    <label for='search-input'>Search:</label>
                    <input id='search-input' name='search-input'>
                </div>
            </div>";

    if !sp_icons_class_names.is_empty() {
        let sp_icons_html_string = generate_html_string_from_classes("SP ICONS", &app_config.selected_sp_icons_css_absolute_file_path,
         "sp-icons", "sp-icons-", "svg", &sp_icons_class_names);
        html += &sp_icons_html_string;
    }

    if !font_awesome_class_names.is_empty() {
        let font_awesome_html_string = generate_html_string_from_classes("FONT AWESOME", &app_config.selected_font_awesome_css_absolute_file_path,
         "fa", "fa-", "svg", &font_awesome_class_names);
        html += &font_awesome_html_string;
    }

    for project_name in sorted_project_names {
        let id = project_name.clone() + "-icons-list";
        let curr_project_dir = project_dirs.get(project_name).unwrap();
        html += &format!("<div class='project-area'>
                            <div class='flex-center'>
                                <h1 class='title margin-right-05'>{}</h1>
                                <span>({})</span>
                            </div>
                            <ul id='{}' class='images-area'>", project_name, curr_project_dir.path, id);

        for (i, image) in curr_project_dir.images.iter().enumerate() {
        html += &format!("<li class='image-container' title='{}'>
                            <div class='extension-stamp color-{}'>
                                <span>{}</span>
                            </div>
                            <img src=\"{}\" alt=\"{}\" />
                            <span>{}</span>
                        </li>
                    ", image.path, image.extension, image.extension, image.path, i, image.name);
        }

        html += "</ul></div>";
    }

    html += "</body>";
    html += &get_css_string(&sp_icons_css_string, &font_awesome_css_string);
    html += &get_javascript_string(sorted_project_names);
    html += "</html>";

    html
}

fn get_css_string(sp_icons_css_string: &String, font_awesome_css_string: &String) -> String {
    let mut css = String::from("<style>
        body {
            background-color: #f1f1f1;
            font-family: Arial, Helvetica, sans-serif;
        }

        ul {
            padding: 0;
            margin: 0;
        }

        .search-container {
            box-shadow: 0 2px 4px rgba(0, 0, 0, 0.4);
            position: sticky;
            top: 0;
            background-color: #d3d3d3;
            padding: 7px;
            z-index: 1;
        }

        .search-container input {
            font-size: 1em;
            height: 1.6em;
            padding: 0 0.3em;
            border-radius: 3px;
            border: none;
            box-shadow: 0 0 3px rgba(0, 0, 0, 0.4);
        }

        .sp-icons {
            font-size: 2.5em;
            margin-left: auto;
            margin-right: auto;
        }

        .fa {
            font-size: 2.5em !important;
            margin-left: auto;
            margin-right: auto;
        }

        .flex-center {
            display: flex;
            align-items: center;
        }

        .margin-right-05 {
            margin-right: 0.5em;
        }

        .project-area {
            margin-bottom: 20px;
        }

        .title+span {
            font-size: 0.9em;
            color: #333333;
        }

        .images-area {
            display: flex;
            flex-wrap: wrap;
            align-items: center;
            column-gap: 10px;
            row-gap: 6px;
        }

        .image-container  {
            position: relative;
            display: flex;
            flex-direction: column;
            word-wrap: break-word;
            width: 4.5em;
        }

        .extension-stamp {
            display: none;
            width: 1.8em;
            height: 1em;
            border-radius: 50%;
            z-index: 1;
            top: -10;
            left: -6;
            position: absolute;
        }

        .extension-stamp > span {
            color: black !important;
        }

        .image-container:hover > .extension-stamp {display: block; }

        .color-svg {
            background-color: #e971e9;
        }

        .color-png {
            background-color: #71e98d;
        }

        .image-container span {
            display: block;
            font-size: 0.8em;
            text-align: center;
            color: #818181;
        }

        .image-container img {
            width: 3em;
            height: auto;
            max-height: 3.5em;
            margin-left: auto;
            margin-right: auto;
        }\n\n");
    if !sp_icons_css_string.is_empty() {
        css += "/*===================>  SP ICONS AREA <===================*/\n\n";
        css += &sp_icons_css_string;
    }
    if !font_awesome_css_string.is_empty() {
        css += "/*===================>  FONT AWESOME AREA <===================*/\n\n";
        css += &font_awesome_css_string;
    }
    css += "</style>";

    css
}

fn generate_html_string_from_classes(section_title: &str, file_path: &str, extra_class: &str, class_prefix: &str, extension: &str, classes: &Vec<String>) -> String {
    let mut html = String::with_capacity(1000);
    html += &format!("<div class='project-area'>
                        <div class='flex-center'>
                            <h1 class='title margin-right-05'>{}</h1>
                            <span>({}) ---- class names are normally prefixed with `{}`</span>
                        </div>", section_title, file_path, class_prefix);

    html += format!("<ul id='{}' class='images-area'>\n", extra_class.to_owned() + "-icons-list").as_str();
    for class in classes {
        html += &format!("<li class='image-container'>
                            <div class='extension-stamp color-{}'>
                                <span>{}</span>
                            </div>
                            <i class='{} {}'></i>
                            <span>{}</span>
                        </li>", extension, extension, extra_class, class, class.strip_prefix("sp-icons-").unwrap_or(class));
    }
    html.push_str("</ul></div>\n");

    html
}

fn parse_special_file(
    special_file_type: SpecialFileType,
    projects_map: &HashMap<String, ProjectDir>,
    app_config: &mut AppConfig)
-> anyhow::Result<Option<(Vec<String>, String)>> {
    let mut file_path = PathBuf::from(special_file_type.get_default_file_path(app_config));
    let mut found_valid_path = true;
    if !file_path.exists() {
        found_valid_path = false;
        if let Some(relative_path) = special_file_type.get_relative_file_path(app_config) {
            for project in projects_map.values() {
                let file_path_str = join_paths(&project.path, relative_path, "");
                file_path = PathBuf::from(file_path_str);
                if file_path.exists() {
                    found_valid_path = true;
                    break;
                }
            }
        }
   }

   if !found_valid_path {
       return Ok(None);
   }

   let file_path_str = match file_path.to_str() {
       Some(file_path_str) => file_path_str,
       None => return Err(anyhow!("Failed to convert file path to string".red())),
   };
   special_file_type.set_selected_file_path(file_path_str, app_config);

   let reader = BufReader::new(File::open(&file_path).context(
       format!("specified file path `{}` for {} is not valid", file_path.to_string_lossy(), special_file_type.get_file_title()).red())?);

   let mut content = String::with_capacity(special_file_type.get_approximate_file_size_bytes());
   let mut class_names = Vec::with_capacity(150);
   for line in reader.lines() {
       let mut line = line.context(format!("Failed to read a line, while parsing {}", special_file_type.get_file_title()).red())?;
       match special_file_type {
           SpecialFileType::SpIconsCss => parse_sp_icons_css(&mut line, &mut class_names, &mut content, app_config),
           SpecialFileType::FontAwesomeCss => parse_font_awesome_css(&mut line, &mut class_names, &mut content, app_config),
       }
   }

   Ok(Some((class_names, content)))
}

fn parse_sp_icons_css(line: &mut String, class_names: &mut Vec<String>, content: &mut String, app_config: &AppConfig) {
    let mut start_index = 0;
    
    while let Some(index) = line[start_index..].find("url('") {
        let relative_path_start = start_index + index + 5;
        if let Some(index_end) = &line[relative_path_start..].find("')") {
            let relative_path_end = relative_path_start + index_end + 2;
            let absolute_path = join_paths(&app_config.sp_icons_css_default_file_dir, &line[relative_path_start..relative_path_end], "/");
            line.replace_range(relative_path_start..relative_path_end, &absolute_path);
            start_index = relative_path_end;
        } else {
            break;
        }
    }
    
    if let Some(class_name) = line.strip_suffix(":before {") {
        class_names.push(class_name[1..].to_owned());
    }
    
    content.push_str(&line);
    content.push('\n');
}


fn parse_font_awesome_css(line: &mut String, class_names: &mut Vec<String>, content: &mut String, app_config: &AppConfig) {
    let mut start_index = 0;
    
    while let Some(index) = line[start_index..].find("url('") {
        let relative_path_start = start_index + index + 5;
        if let Some(index_end) = &line[relative_path_start..].find("')") {
            let relative_path_end = relative_path_start + index_end + 2;
            let absolute_path = join_paths(&app_config.font_awesome_css_default_file_dir, &line[relative_path_start..relative_path_end], "/");
            line.replace_range(relative_path_start..relative_path_end, &absolute_path);
            start_index = relative_path_end;
        } else {
            break;
        }
    }
    
    if let Some(class_name) = line.strip_suffix(":before {") {
        class_names.push(class_name[1..].to_owned());
    }
    
    content.push_str(&line);
    content.push('\n');
}

fn get_htdocs_path() -> Option<String> {
    let os = env::consts::OS;

    match os {
        "windows" => {
            let default_path_str = "C:/xampp/htdocs";
            let xampp_htdocs_path = PathBuf::from(default_path_str);
            if xampp_htdocs_path.exists() {
                return Some(default_path_str.to_owned());
            }
        }
        "macos" => {
            let default_path_str = "/Applications/XAMPP/xamppfiles/htdocs";
            let xampp_htdocs_path = PathBuf::from(default_path_str);
            if xampp_htdocs_path.exists() {
                return Some(default_path_str.to_owned());
            }
        }
        "linux" => {
            let default_path_str = "/opt/lampp/htdocs"; //TODO: check
            let xampp_htdocs_path = PathBuf::from("/opt/lampp/htdocs");
            if xampp_htdocs_path.exists() {
                return Some(default_path_str.to_owned());
            }
        }
        _ => {return None}
    }

    None
}

fn join_paths(base_absolute_path: &str, relative_path: &str, connective_str: &str) -> String {
    let concated = format!("{}{}{}", base_absolute_path, connective_str, relative_path);
    concated.replace("\\", "/").to_owned()
}

fn open_generated_file_in_the_browser(app_config: &AppConfig) {
    // Open the HTML file in the default browser
    if cfg!(target_os = "windows") {
        // Windows command
        Command::new("cmd")
            .args(&["/C", "start", "", &app_config.output_file_path])
            .spawn()
            .expect("Failed to open HTML file in the browser");
    } else if cfg!(target_os = "macos") {
        // macOS command
        Command::new("open")
            .arg(&app_config.output_file_path)
            .spawn()
            .expect("Failed to open HTML file in the browser");
    } else {
        // Linux command
        Command::new("xdg-open")
            .arg(&app_config.output_file_path)
            .spawn()
            .expect("Failed to open HTML file in the browser");
    }
}

fn write_to_file(contents: String, app_config: &AppConfig) -> anyhow::Result<()>{
    let mut file = File::create(&app_config.output_file_path).map_err(|_| anyhow!("Failed to create test.html"))?;
    file.write_all(contents.as_bytes()).context("Failed to write to file")?;
    Ok(())
}

#[derive(Debug)]
struct AppConfig {
    // folder that contains the projects in which we want to search for Icons. By default it is the path to htdocs
    pub root_folder : String, 

    // name of the file that will be generated
    pub output_file_name : String,

    // full path of generated file - created automatically from the output_file_name
    pub output_file_path : String,

    // path to the directory that contains the sp-icons css file
    pub sp_icons_css_default_file_dir : String,

    // full path of the sp-icons css file
    pub sp_icons_css_default_absolute_file_path : String,

    // the absolute file path that was used for parsing
    pub selected_sp_icons_css_absolute_file_path : String,

    // relative path to file, from inside the project folder
    pub font_awesome_css_relative_file_path : String,

    // path to the directory that contains the font-awesome css file
    pub font_awesome_css_default_file_dir : String,

    // full path of the font-awesome css file
    pub font_awesome_css_default_absolute_file_path : String,

    // the absolute file path that was used for parsing
    pub selected_font_awesome_css_absolute_file_path : String,

    // names of folders that should be ignored in each project, like node_modules, bower_components, ...
    pub irrelevant_dir_names: Vec<&'static str>,

    // extensions that we want to search for, like svg, png, ...
    pub relevant_extensions: Vec<&'static str>,
}

#[derive(Debug, Default, Clone)]
struct ProjectDir {
    pub name: String,
    pub path: String,
    pub images: Vec<Img>,
}

#[derive(Debug, Clone)]
struct Img {
    pub name: String,
    pub path: String,
    pub extension: String,
}

enum SpecialFileType {
    SpIconsCss,
    FontAwesomeCss,
}

impl SpecialFileType {
    pub fn get_file_title(&self) -> &str {
        match self {
            SpecialFileType::SpIconsCss => "sp-icons",
            SpecialFileType::FontAwesomeCss => "font-awesome",
        }
    }

    pub fn get_default_file_path<'a>(&self, app_config: &'a AppConfig) -> &'a String {
        match self {
            SpecialFileType::SpIconsCss => &app_config.sp_icons_css_default_absolute_file_path,
            SpecialFileType::FontAwesomeCss => &app_config.font_awesome_css_default_absolute_file_path,
        }
    }

    pub fn get_relative_file_path<'a>(&self, app_config: &'a AppConfig) -> Option<&'a String> {
        match self {
            SpecialFileType::SpIconsCss => None,
            SpecialFileType::FontAwesomeCss => Some(&app_config.font_awesome_css_relative_file_path),
        }
    }

    pub fn get_approximate_file_size_bytes(&self) -> usize {
        match self {
            SpecialFileType::SpIconsCss => 1100,
            SpecialFileType::FontAwesomeCss => 38000,
        }
    }

    pub fn set_selected_file_path(&self, file_path: &str, app_config: &mut AppConfig) {
        match self {
            SpecialFileType::SpIconsCss => app_config.selected_sp_icons_css_absolute_file_path = file_path.to_owned(),
            SpecialFileType::FontAwesomeCss => app_config.selected_font_awesome_css_absolute_file_path = file_path.to_owned(),
        }
    }
}

impl AppConfig {
    pub fn init() -> anyhow::Result<Self> {
        let command_args: String = env::args().skip(1).collect::<Vec<String>>().join(" ");
        let root_folder = {
            if !command_args.trim().is_empty() {
                let command_args_buf = PathBuf::from(&command_args);
                if !command_args_buf.exists() {
                    return Err(anyhow!("The provided path doesn't appear to be valid.".red()));
                }
                command_args
            } else {
                get_htdocs_path().context("Default htdocs path not found on your system. Provide a root folder manually as a command line argument.".red())?
            }
        };

        let output_file_name = "icons_report_generated.html".to_owned();
        let desktop_dir = dirs::desktop_dir();
        let mut output_file_path: PathBuf;
        if let Some(dir) = desktop_dir {
            output_file_path = dir;
        } else {
            output_file_path = Path::new(".").to_path_buf();
        }
        output_file_path.push(&output_file_name.clone());
        let output_file_path = match output_file_path.to_str() {
            Some(path) => path.to_owned(),
            None => return Err(anyhow!("Unable to convert output file path to string")),
        };

        let sp_icons_default_css_file_dir = root_folder.clone() + "/mega-commons-angular-js/assets/fonts/sp-icons";
        let font_awesome_relative_file_dir = "/bower_components/components-font-awesome/css";
        let font_awesome_default_css_dir = root_folder.clone() + "/mega-commons-angular-js" + font_awesome_relative_file_dir.clone();

        Ok (Self { 
            root_folder: root_folder.clone(),
            output_file_name,
            output_file_path,
            sp_icons_css_default_file_dir: sp_icons_default_css_file_dir.clone(),
            sp_icons_css_default_absolute_file_path: sp_icons_default_css_file_dir + "/style.css",
            selected_sp_icons_css_absolute_file_path: String::new(),
            font_awesome_css_relative_file_path: font_awesome_relative_file_dir.to_owned() + "/font-awesome.css",
            font_awesome_css_default_file_dir: font_awesome_default_css_dir.to_owned(),
            font_awesome_css_default_absolute_file_path: font_awesome_default_css_dir + "/font-awesome.css",
            selected_font_awesome_css_absolute_file_path: String::new(),
            relevant_extensions: vec!["svg", "png"],
            irrelevant_dir_names: vec![
                    "bower_components",
                    "node_modules",
                    "vendor",
                    "dist",
                    "api",
                    "app",
                ],
        })
    }
}
