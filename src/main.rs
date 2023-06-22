mod message_printer;

use std::{fs::{File}, io::{Write, BufReader, BufRead}, collections::HashMap, process::Command, path::{PathBuf, Path}, env, time::Instant};

use anyhow::{Context, anyhow};
use colored::*;
use chrono::{DateTime, offset::{Utc, FixedOffset}};
use walkdir::WalkDir;

use message_printer::*;

// Application version, to be displayed at startup and on the webpage
pub const VERSION_ID : &str = "v1.0.0"; 

fn main() -> anyhow::Result<()> {
    let instant = Instant::now();

    // Only on windows, it is required to enable a virtual terminal environment, so that the colors will display correctly
    #[cfg(target_os = "windows")]
    control::set_virtual_terminal(true).unwrap();

    println!("img-dumper {}\n", VERSION_ID);

    let program_args = parse_args()?;
    let program_args = match program_args {
        Some(value) => value,
        None => {
            message_printer::print_whole_help_message();
            return Ok(());
        }
    };

    let mut app_config = AppConfig::init(program_args)?;
    println!("Root folder: {}\n", app_config.root_dir);

    print!("Parsing projects... ");
    let mut projects_map = traverse_root_dir_and_make_project_map(&app_config);
    projects_map.retain(|_, project_dir| !project_dir.images.is_empty());
    if projects_map.is_empty() {
        println!("{}", "No icons could be found for any projects".yellow());
    } else {
        println!("{} ({} found)", "OK".green(), projects_map.len());
    }
    projects_map.values_mut().for_each(|f| f.images.sort_by(|a, b| a.name.cmp(&b.name)));
    let mut sorted_project_names: Vec<String> = projects_map.keys().into_iter().map(|k| k.clone()).collect();
    sorted_project_names.sort();

    let (sp_icons_class_names, sp_icons_css_string) = {
        if app_config.command_line_args.is_basic {
            (Vec::new(), String::new())
        } else {
            print!("Parsing sp-icons... ");
            match parse_special_file(SpecialFileType::SpIconsCss, &projects_map, &mut app_config)? {
                Some((sp_icons_class_names, sp_icons_css_string)) => {
                    println!("{}", "OK".green());
                    (sp_icons_class_names, sp_icons_css_string)
                },
                None => {
                    println!("{}", "No sp-icons file found".yellow());
                    (Vec::new(), String::new())
                }
            }       
        }
    };
    let (font_awesome_class_names, font_awesome_css_string) = {
        if app_config.command_line_args.is_basic {
            (Vec::new(), String::new())
        } else {
            print!("Parsing font-awesome... ");
            match parse_special_file(SpecialFileType::FontAwesomeCss, &projects_map, &mut app_config)? {
                Some((font_awesome_class_names, font_awesome_css_string)) => {
                    println!("{}", "OK".green());
                    (font_awesome_class_names, font_awesome_css_string)
                },
                None => {
                    println!("{}", "No font-awesome file found".yellow());
                    (Vec::new(), String::new())
                }
            }
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

    for entry in WalkDir::new(&app_config.root_dir)
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
                    path: entry_path.to_string_lossy().replace("\\","/"),
                    images: Vec::new(),
                });

            images = vec![];
        }

        let extension = entry_path.extension().unwrap_or_default().to_str().unwrap_or_default().to_lowercase();

        if app_config.relevant_extensions.contains(&extension.as_str()) {
            let name = entry_path.file_stem().unwrap_or_default().to_str().unwrap_or_default();
            let img = Img {
                name: name.to_owned(),
                path: entry_path.to_string_lossy().replace("\\","/"),
                extension: extension.to_owned(),
            };
            images.push(img);
        }
    }

    project_dir.images = images;

    project_dirs
}

fn get_javascript_string(app_config: &AppConfig) -> String {
    let mut js = 
    "<script>
    const inputElement = document.getElementById('search-input');
    inputElement.addEventListener('input', handleSearchChange);

    function handleSearchChange() {
        let input, filter, uls, lis, a, i, span, txtValue;
        input = document.getElementById('search-input');
        filter = input.value.toUpperCase();
        let relevant_lis_count = 0;
        uls = document.getElementsByTagName('ul');
        for (ul of uls) {
            parent_project_area_div = ul.parentElement;
            lis = ul.getElementsByTagName('li');
            for (i = 0; i < lis.length; i++) {
                extensionSpan = lis[i].getElementsByTagName('span')[0];
                extensionValue = extensionSpan.textContent || extensionSpan.innerText;
                nameSpan = lis[i].getElementsByTagName('span')[1];
                nameValue = nameSpan.textContent || nameSpan.innerText;
                if (nameValue.toUpperCase().indexOf(filter) > -1 && currentlySelectedExtensions.includes(extensionValue.toLowerCase())) {
                    lis[i].style.display = '';
                    relevant_lis_count += 1;
                } else {
                    lis[i].style.display = 'none';
                }
            }
            if (relevant_lis_count == 0) {
                parent_project_area_div.style.display = 'none';
            } else {
                parent_project_area_div.style.display = 'block';
            }
            relevant_lis_count = 0;
        }
    }

    // handler to copy paths from titles of <li> elements
    document.addEventListener('click', handleLiClick);
    function handleLiClick($event) {
        let target = $event.target;
        if (target.parentElement.tagName.toLowerCase() === 'li') {
            target = target.parentElement;
        }
        if (target.tagName.toLowerCase() === 'li') {
            let titleValue = target.getAttribute('title');
            if (!titleValue) {
                return;
            }
            titleValue = titleValue.substring(0, titleValue.lastIndexOf('/'));
            navigator.clipboard.writeText(titleValue)
                .then(() => {
                    console.log('Text copied to clipboard: ' + titleValue);
                    const copyNotification = document.getElementById('copy-notification');
                    // document.getElementById('copy-notification').style.display = 'flex';
                    copyNotification.classList.add('show');
                    setTimeout(() => {
                        copyNotification.classList.remove('show');
                        // document.getElementById('copy-notification').style.display = 'none';
                    }, 1000);
                })
                .catch((error) => {
                    console.error('Error copying text to clipboard:', error);
                });
        }
    }

    function toggleProjectArea($event) {
        let element = $event.currentTarget;
        let downChild = element.querySelector('span.down');
        let upChild = element.querySelector('span.up');
        let ul = element.parentElement.parentElement.querySelector('ul.images-area');
        console.log('ul: ', ul);

        // down arrow is showing in the beginning
        if (getComputedStyle(downChild).display !== 'none') {
            downChild.style.display = 'none';
            ul.style.display = 'none';
        } else {
            downChild.style.display = '';
            ul.style.display = '';
        }

        if (getComputedStyle(upChild).display !== 'none') {
            upChild.style.display = 'none';
        } else {
            upChild.style.display = '';
        }
    } 

    function handleCheckboxChange(event) {
        const checkbox = event.currentTarget;
        const checkboxValue = checkbox.value;
        const isChecked = checkbox.checked;
      
        if (isChecked) {
          currentlySelectedExtensions.push(checkboxValue);
        } else {
          currentlySelectedExtensions = currentlySelectedExtensions.filter(
            (ext) => ext !== checkboxValue
          );
        }
      
        let relevant_lis_count = 0;
        const uls = document.getElementsByTagName('ul');

        inputFilter = document.getElementById('search-input').value.toUpperCase();

        for (const ul of uls) {
          const parent_project_area_div = ul.parentElement;
          const lis = ul.getElementsByTagName('li');
      
          for (const li of lis) {
            extensionSpan = li.getElementsByTagName('span')[0];
            extensionValue = extensionSpan.textContent || extensionSpan.innerText;
            nameSpan = li.getElementsByTagName('span')[1];
            nameValue = nameSpan.textContent || nameSpan.innerText;

            if (currentlySelectedExtensions.includes(extensionValue.toLowerCase()) && (!nameValue || nameValue.toUpperCase().indexOf(inputFilter) > -1)) {
              if (window.getComputedStyle(li).display === 'none') {
                li.style.display = '';
              }
              relevant_lis_count += 1;
            } else {
                if (window.getComputedStyle(li).display !== 'none') {
                    li.style.display = 'none';
                }
            }
          }
      
          if (relevant_lis_count === 0) {
            parent_project_area_div.style.display = 'none';
          } else {
            parent_project_area_div.style.display = 'block';
          }
      
          relevant_lis_count = 0;
        }
      }
    ".to_owned();

    let joined_values = app_config
    .relevant_extensions
    .iter()
    .map(|value| format!("\"{}\"", value))
    .collect::<Vec<String>>()
    .join(",");

    js.push_str(&format!("
    let currentlySelectedExtensions = [{}];
    ",joined_values));

    js.push_str("</script>");

    js
}

fn generate_html_page_as_string(
        project_dirs: &HashMap<String, ProjectDir>,
        sorted_project_names: &Vec<String>,
        sp_icons_class_names: &Vec<String>,
        sp_icons_css_string: &String,
        font_awesome_class_names: &Vec<String>,
        font_awesome_css_string: &String,
        app_config: &AppConfig) -> String {
    let mut html = String::from("<html lang='en'> <head> <title>Spectre icons</title> </head> <body>");
    
    html += 
    "<div class='search-container'>
        <div>
            <label for='search-input'>Search:</label>
            <input id='search-input' name='search-input'>
        </div>
        <div class='flex-center flex-1'>";
        for extension in &app_config.relevant_extensions {
            html += &format!(
            "<div class='checkbox-item'>
                <input type='checkbox' id='checkbox-{}' name='checkbox-{}' value='{}' onchange='handleCheckboxChange(event)' checked>
                <label for='checkbox-{}'>{}</label>
            </div>", extension, extension, extension, extension, extension);
        }
    html += &format!(
    "   </div>
            <div class='date-marker-area'>
                <span>{}</span>
                <span>Generated at:</span>
                <span>{}</span>
            </div>
    </div>", VERSION_ID, app_config.exec_date_time.format("%d/%m/%Y - %H:%M:%S").to_string());

    html +=
    "<div id='copy-notification' class='fade'>
        <span> Copied path to clipboard!</span>
    </div>";

    if !sp_icons_class_names.is_empty() {
        let sp_icons_html_string = generate_html_string_from_classes("sp-icons", &app_config.selected_sp_icons_css_absolute_file_path,
         "sp-icons", "sp-icons-", "svg", &sp_icons_class_names);
        html += &sp_icons_html_string;
    }

    if !font_awesome_class_names.is_empty() {
        let font_awesome_html_string = generate_html_string_from_classes("font-awesome", &app_config.selected_font_awesome_css_absolute_file_path,
         "fa", "fa-", "svg", &font_awesome_class_names);
        html += &font_awesome_html_string;
    }

    for project_name in sorted_project_names {
        let curr_project_dir = project_dirs.get(project_name).unwrap();
        html += &format!("<div class='project-area'>
                            <div class='flex-center'>
                                <div class='name-arrow-container' onclick='toggleProjectArea(event)'>
                                    <span class='down arrow-utf-8'>&#9660</span>
                                    <span class='up arrow-utf-8' style='display: none'>&#9650</span>
                                    <h1 class='title margin-right-05'>{}</h1>
                                </div>
                                <span>({})</span>
                            </div>
                            <ul class='images-area'>", project_name, curr_project_dir.path);

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
    html += &get_javascript_string(app_config);
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
            position: sticky;
            top: 0;
            z-index: 1;
            display: flex;
            align-items: center;
            column-gap: 3em;
            box-shadow: 0 2px 4px rgba(0, 0, 0, 0.4);
            background-color: #d3d3d3;
            padding: 6px;
        }

        .search-container input {
            font-size: 1em;
            height: 1.6em;
            padding: 0 0.3em;
            border-radius: 3px;
            border: none;
            box-shadow: 0 0 3px rgba(0, 0, 0, 0.4);
        }

        .checkbox-item {
            display: flex;
            width: fit-content; 
            width: -moz-fit-content;
            align-items: center; 
            column-gap: 0.2em;
            cursor: pointer;
        }

        .checkbox-item ~ .checkbox-item {
            margin-left: 0.8em;
        }

        .checkbox-item input {
            width: 1em;
            cursor: pointer;
        }

        .checkbox-item label {
            cursor: pointer;
        }

        .date-marker-area span:first-child {
            color: #3a3a3a;
            font-size: 0.85em;
            font-style: italic;
        }

        .date-marker-area span:last-child {
            border: #fafafa; 
            border-style: inset; 
            padding: 0.1em; 
            border-radius-top: 5px; 
        }

        #copy-notification {
            position: fixed; 
            z-index: 1; 
            right: 10; 
            bottom: 15;
            display: flex; 
            align-items: center;
            width: fit-content; 
            width: -moz-fit-content;
            background-color: rgba(0,0,0,0.8); 
            border-radius: 16px; 
            // display: none;
        }

        #copy-notification span:first-child {
            color: white; 
            padding: 0.5em 1em; 
            font-style: italic;
        }

        .fade {
            opacity: 0;
            transition: opacity 0.3s ease-in-out;
        }
        
        .fade.show {
            opacity: 1;
        }

        .name-arrow-container {
            cursor: pointer;
            display: flex;
            align-items: center;
        }

        .arrow-utf-8 {
            font-size: 0.8em;
            margin-top: 0.3em;
            padding: 0.5em;
        }

        .arrow-utf-8.up {
            font-size: 1em;
            padding: 0.4em;
        }

        span.up.arrow-utf-8 {
            color: #1fbc1f;
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

        .flex-1 {
            flex: 1;
        }

        .margin-right-05 {
            margin-right: 0.5em;
        }

        .project-area {
            margin-bottom: 20px;
        }

        .title {
            margin-top: 0.3em;
            margin-bottom: 0.3em;
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

        .image-container[title] {
            cursor: copy; 
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
                            <div class='name-arrow-container' onclick='toggleProjectArea(event)'>
                                <span class='down arrow-utf-8'>&#9660</span>
                                <span class='up arrow-utf-8' style='display: none'>&#9650</span>
                                <h1 class='title margin-right-05'>{}</h1>
                            </div>
                            <span>({}) ---- class names are normally prefixed with `{}`</span>
                        </div>", section_title, file_path, class_prefix);

    html += "<ul class='images-area'>\n";
    for class in classes {
        html += &format!("<li class='image-container'>
                            <div class='extension-stamp color-{}'>
                                <span>{}</span>
                            </div>
                            <i class='{} {}'></i>
                            <span>{}</span>
                        </li>", extension, extension, extra_class, class, class.strip_prefix(class_prefix).unwrap_or(class));
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

   class_names.sort();

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
            for path_str in ["/opt/lampp/htdocs", "/var/www/html"] {
                let xampp_htdocs_path = PathBuf::from(path_str);
                if xampp_htdocs_path.exists() {
                    return Some(path_str.to_owned());
                }
            }
        }
        _ => {return None}
    }

    None
}

fn parse_args() -> anyhow::Result<Option<CommandLineArgs>> {
    let line = env::args().skip(1).collect::<Vec<String>>().join(" ");
    let mut commands = line.split("--");

    if line.trim().starts_with("--") {
        //ignoring the empty first element that is caused by splitting
        commands.next();
    }

    let (mut dir, mut target, mut name, mut is_basic) = (None, None, None, false);
    for command in commands {
        let (command_name, arguments) = match command.find(" ") {
            Some(index) => command.split_at(index),
            None => (command.trim(), "")
        };
        if command_name == Argument::Dir.get_name() {
            let mut path = arguments.trim().replace("\\", "/");
            path = path.strip_prefix('"').unwrap_or(&path).strip_suffix('"').unwrap_or(&path).to_owned();
            if path.is_empty() {
                println!("{}", Argument::Dir.get_help_msg());
                return Err(anyhow!("No argument provided for --dir".red()));
            }
            dir = Some(path);
        } else if command_name == Argument::Target.get_name() {
            let path = arguments.trim();
            if path.is_empty() {
                println!("{}", Argument::Target.get_help_msg());
                return Err(anyhow!("No argument provided for --target".red()));
            }
            target = Some(path.to_owned());
        } else if command_name == Argument::Name.get_name() {
            let _name = arguments.trim();
            if _name.is_empty() {
                println!("{}", Argument::Name.get_help_msg());
                return Err(anyhow!("No argument provided for --name".red()));
            }
            name = Some(_name.to_owned());
        } else if command_name == Argument::Basic.get_name() {
            let flag = arguments.trim();
            if !flag.is_empty() {
                println!("Warning: {}\n", format!("Ignoring argument for --{}",Argument::Basic.get_name()).yellow());
            }
            is_basic = true;
        } else if command_name == Argument::Help.get_name() {
            return Ok(None);
        } else if !command_name.trim().is_empty() {
            return Err(anyhow!(format!("Unknown command: {}", command_name).red()));
        }
    }

    let program_args = CommandLineArgs { dir, target, name, is_basic };

    Ok(Some(program_args))
}

pub fn get_trimmed_if_not_empty(str: &str) -> Option<String> {
    let str = str.trim();
    if str.is_empty() {None}
    else {Some(str.to_owned())}
}

fn join_paths(base_absolute_path: &str, relative_path: &str, connective_str: &str) -> String {
    let concated = format!("{}{}{}", base_absolute_path, connective_str, relative_path);
    concated.replace("\\", "/").to_owned()
}

fn convert_to_absolute(s: &str) -> String {
    let p = Path::new(s);
    if p.is_absolute() {
        return s.replace("\\", "/");
    }

    // The "canonicalize" function, (at least on windows) seems to put the weird prefix
    // "\\?\" before the path and it also puts forward slashes that we want to convert for compatibility.  
    if let Ok(buf) = std::fs::canonicalize(p) {
        let str_path = buf.to_str().unwrap();
        str_path.strip_prefix(r"\\?\").unwrap_or(str_path).replace("\\", "/")
    } else {
        s.replace("\\", "/")
    }
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
    let mut file = File::create(&app_config.output_file_path).map_err(|_| anyhow!("Failed to create the .html file".red()))?;
    file.write_all(contents.as_bytes()).context("Failed to write to file".red())?;
    Ok(())
}

#[derive(Debug)]
struct AppConfig {
    pub command_line_args: CommandLineArgs,

    // the date and time when the program was executed and the html file was generated
    pub exec_date_time: DateTime<FixedOffset>,

    // folder that contains the projects in which we want to search for Icons. By default it is the path to htdocs
    pub root_dir : String, 

    // name of the file that will be generated
    pub output_file_name : String,

    // full path of generated file
    pub output_file_path : String,

    // path to the directory that contains the sp-icons css file
    pub sp_icons_css_default_file_dir : String,

    // default path of the sp-icons css file
    pub sp_icons_css_default_absolute_file_path : String,

    // the file path that was used for parsing the sp icons css file
    pub selected_sp_icons_css_absolute_file_path : String,

    // relative path to sp icons css file, from inside the project folder
    pub font_awesome_css_relative_file_path : String,

    // path to the directory that contains the font-awesome css file
    pub font_awesome_css_default_file_dir : String,

    // default path of the font-awesome css file
    pub font_awesome_css_default_absolute_file_path : String,

    // the absolute file path that was used for parsing the font-awesome css file
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

#[derive(Debug, Default)]
struct CommandLineArgs {
    pub dir: Option<String>,
    pub target: Option<String>,
    pub name: Option<String>,
    pub is_basic: bool,
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
    pub fn init(args: CommandLineArgs) -> anyhow::Result<Self> {
        let root_dir = {
            if let Some(dir) = &args.dir {
                dir.clone()
            } else {
                if let Some(x) = get_htdocs_path() {
                    x
                } else {
                    return Err(anyhow!("Unable to find htdocs folder and no custom root directory provided".red()));
                }
            }
        };
        let root_dir = convert_to_absolute(&root_dir);

        if !Path::new(&root_dir).exists() {
            return Err(anyhow!(format!("The (root) directory '{}' does not exist", root_dir).red()));
        }

        let mut output_file_name = 
            if let Some(name) = &args.name {
                name.to_owned()
            } else {
                "icons_report_generated".to_owned()
            };
        output_file_name.push_str(".html");

        let output_file_path =
            if let Some(target) = &args.target {
                let path = PathBuf::from(target);
                if !path.is_dir() {
                    return Err(anyhow!(format!("The target '{}' is not a directory", target).red()));
                }
                path
            } else {
                let desktop_dir = dirs::desktop_dir();
                if let Some(dir) = desktop_dir {
                    dir
                } else {
                    Path::new(".").to_path_buf()
                }
            };
        let output_file_path = convert_to_absolute(&join_paths(&output_file_path.to_string_lossy(), &output_file_name, "/"));

        let sp_icons_default_css_file_dir = root_dir.clone() + "/mega-commons-angular-js/assets/fonts/sp-icons";
        let font_awesome_relative_file_dir = "/bower_components/components-font-awesome/css";
        let font_awesome_default_css_dir = root_dir.clone() + "/mega-commons-angular-js" + font_awesome_relative_file_dir.clone();

        Ok (Self { 
            command_line_args: args,
            exec_date_time: Utc::now().with_timezone(&FixedOffset::east_opt(3 * 3600).unwrap()),
            root_dir,
            output_file_name,
            output_file_path,
            sp_icons_css_default_file_dir: sp_icons_default_css_file_dir.clone(),
            sp_icons_css_default_absolute_file_path: sp_icons_default_css_file_dir + "/style.css",
            selected_sp_icons_css_absolute_file_path: String::new(),
            font_awesome_css_relative_file_path: font_awesome_relative_file_dir.to_owned() + "/font-awesome.css",
            font_awesome_css_default_file_dir: font_awesome_default_css_dir.to_owned(),
            font_awesome_css_default_absolute_file_path: font_awesome_default_css_dir + "/font-awesome.css",
            selected_font_awesome_css_absolute_file_path: String::new(),
            relevant_extensions: vec!["svg", "png", "jpg", "jpeg", "gif", "bmp", "ico"],
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
