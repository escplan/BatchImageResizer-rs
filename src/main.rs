#![allow(dead_code)]

// no console window on windows
#![windows_subsystem = "windows"]

use std::{
    fs,
    time,
    thread,
    sync::mpsc,
};

// files dialog
extern crate nfd;
use nfd::Response;

// how many cpus, how many image resizing threads
extern crate num_cpus;

// webview for gui
use web_view::*;

mod imgproc2;
use imgproc2::*;

fn main() {
    // inter thread communication ref:https://www.jianshu.com/p/64d75041b8c0
    // two channels, launch and working for communication
    let (tx_launch, rx_launch) = mpsc::channel();
    let (tx_working, rx_working) = mpsc::channel();
    let senders = Senders {
        tx_launch: tx_launch.clone(),
        tx_working: tx_working.clone(),
    };

    // html for webview front
    let html = format!(
        r#"<!DOCTYPE html>
        <html>
        <head>
            <meta http-equiv="X-UA-Compatible" content="IE=edge">
            <meta charset="UTF-8">
            {styles}
        </head>
        <body>
            {htmlclip}
            {scripts}
        </body>
        </html>
        "#
        ,
        htmlclip    =  include_str!("../html/index.html"),
        styles      = inline_style(include_str!("../html/semantic.min.css"))
                    + &inline_style(include_str!("../html/styles.css"))
        ,
        scripts     = inline_script(include_str!("../html/jquery-3.slim.min.js"))
                    + &inline_script(include_str!("../html/semantic.js"))
                    + &inline_script(include_str!("../html/vue.js"))
                    + &inline_script(include_str!("../html/app.js"))
        ,
    );

    // webview
    // user_data as Senders for inter thread communication
    let wv = web_view::builder()
        .title("BatchImageResizer-rs")
        .content(Content::Html(html))
        .size(370, 700)
        .resizable(false)
        .debug(false) 
        .user_data(senders) 
        .invoke_handler(invoke_handler)
        .build()
        .unwrap();

    // thread for launch works
    thread::spawn(move || {
        // inter thread communication for launch
        for works in rx_launch {
            let _tx = tx_working.clone();
            // new thread for image resizing
            // do not processing image in main thread
            // that will make webview no response
            thread::spawn(move || {
                tasks_main(works.files, works.paras, _tx);
            });
        };
    });

    // thread for catching progress
    // two mut for count works process success and fails count
    let mut works_process_n:u32 = 0;
    let mut fail_img_n:u32 = 0;

    // a handle for switch to main thread to renew webview
    let handle_working = wv.handle();

    // new thread for catch inter thread communication for size resizing process to main thread
    thread::spawn(move || {
        // limit for webview renew, two requests >150ms
        let renewlimit = 150;
        let mut renew: bool = false;
        let mut keytime = time::SystemTime::now();

        // inter thread communication for size resizing process to main thread
        for msg in rx_working {
            // catch cmds from webview front
            let cmds:Vec<&str> = msg.split(CMDS_JOINSIGN).collect();
            match cmds[0] {
                "reset" => {
                    works_process_n = 0;
                    fail_img_n = 0;
                },
                "working" => {
                    let _count = cmds[1].parse::<u32>().unwrap();
                    if _count > works_process_n { works_process_n = _count };
                },
                "err" => {
                    let _fails = cmds[1].parse::<u32>().unwrap();
                    if _fails > fail_img_n { fail_img_n = _fails };
                    renew = true;
                },
                "done" => {
                    renew = true;
                }
                _ => {}
            };

            // check for renew limit
            let _now = keytime.elapsed().unwrap();
            let _t = _now.as_millis();
            if _t > renewlimit {
                keytime = time::SystemTime::now();
                renew = true;
            }

            // switch to main thread to renew if renew is true
            if renew {
                handle_working.dispatch(move |wv| {
                    let cmds:Vec<&str> = msg.split(CMDS_JOINSIGN).collect();
                    match cmds[0] {
                        "working" => {
                            let _img_name = cmds[2];
                            // flashmessage for currect working file's name
                            wv.eval(&format!("vue.flashmessage({:?})", _img_name))?;
                            // resize_process for progress renew
                            wv.eval(&format!("vue.resize_process({:?})", works_process_n))?;
                        },
                        "err" => {
                            let _img_name = cmds[2];
                            let _err_type = cmds[3];
                            let _s = vec!["\nErr", _img_name, _err_type].join("\n");
                            wv.eval(&format!("vue.failures = {}", fail_img_n))?;
                            wv.eval(&format!("vue.showmessage({:?})", _s))?;
                            wv.eval(&format!("scrollToBottom()",))?;
                        },
                        "done" => {
                            // if done, recheck process count
                            works_process_n = get_count();
                            wv.eval(&format!("vue.resize_process({:?})", works_process_n))?;

                            // display total time
                            let _elapsed = format!("{}", cmds[1]);
                            wv.eval(&format!("vue.flashmessage({:?})", _elapsed))?;

                            // recheck fails count
                            wv.eval(&format!("vue.failures = {}", fail_img_n))?;
                            wv.eval(&format!("vue.doisdone()",))?;
                            wv.eval(&format!("scrollToBottom()",))?;
                        },
                        _ => {},
                    };

                    Ok(())
                }).unwrap();

                renew = false;
            }
        };
    });

    wv.run().unwrap();
}

fn invoke_handler(wv: &mut WebView<Senders>, arg: &str) -> WVResult {
    // CMDS_JOINSIGN for split command and args, it's not 100% safe, but 99.99999%
    let args:Vec<&str>  = arg.split(CMDS_JOINSIGN).collect();

    match args[0] {
        "openMulti" => {
            let result = nfd::dialog_multiple().open().unwrap_or_else(|e| {
                panic!(e);
            });
            match result {
                Response::Okay(file_path) => println!("File path = {:?}", file_path),
                Response::OkayMultiple(files) => {
                    // FILES_JOINSIGN for paths to string, it's not 100% safe, but 99.99999%
                    let s = files.join(FILES_JOINSIGN);
                    let l = files.len();
                    wv.eval(&format!("vue.sourcefolder = null"))?;
                    wv.eval(&format!("vue.fileliststring = {:?}", s))?;
                    wv.eval(&format!("vue.filelistlen = {}", l))?;
                    wv.eval(&format!("vue.process_n = 0", ))?;
                    wv.eval(&format!("vue.failures = 0", ))?;
                    wv.eval(&format!("vue.isResizeDisabled = false"))?;
                    wv.eval(&format!("vue.checkResizeDisabled()"))?;
                    wv.eval(&format!("vue.checkExportFolder()"))?;
                    wv.eval(&format!("vue.isdone = null",))?;
                },
                Response::Cancel => println!("User canceled"),
            };
        },

        "openFolder" => {
            let result = nfd::open_pick_folder(None).unwrap_or_else(|e| {
                panic!(e);
            });

            match result {
                Response::Okay(file_path) => {
                    let mut files:Vec<String> = Vec::new();

                    if args[1] == "includeSubfolder" {
                        let ret = seekdir_deeper(&file_path);
                        files.extend(ret.0.iter().cloned());
                    } else {
                        let ret = seekdir(&file_path);
                        files.extend(ret.0.iter().cloned());
                    }

                    let s = files.join(FILES_JOINSIGN);
                    let l = files.len();
                    wv.eval(&format!("vue.sourcefolder = {:?}", file_path))?;
                    wv.eval(&format!("vue.fileliststring = {:?}", s))?;
                    wv.eval(&format!("vue.filelistlen = {}", l))?;
                    wv.eval(&format!("vue.process_n = 0", ))?;
                    wv.eval(&format!("vue.failures = 0", ))?;
                    wv.eval(&format!("vue.isResizeDisabled = false"))?;
                    wv.eval(&format!("vue.checkExportFolder()"))?;
                    wv.eval(&format!("vue.isdone = null",))?;
                },
                Response::Cancel => println!("User canceled"),
                _ => (),
            }
        },

        // check how many files in folder, and within or without includeSubfolder
        "checkFolder" => {
            let file_path = args[2];
            let mut files:Vec<String> = Vec::new();

            if args[1] == "includeSubfolder" {
                let ret = seekdir_deeper(&file_path);
                files.extend(ret.0.iter().cloned());
            } else {
                let ret = seekdir(&file_path);
                files.extend(ret.0.iter().cloned());
            }

            let s = files.join(FILES_JOINSIGN);
            let l = files.len();
            wv.eval(&format!("vue.fileliststring = {:?}", s))?;
            wv.eval(&format!("vue.filelistlen = {}", l))?;
            wv.eval(&format!("vue.process_n = 0", ))?;
            wv.eval(&format!("vue.failures = 0", ))?;
            wv.eval(&format!("vue.checkExportFolder()"))?;
            wv.eval(&format!("vue.isdone = null",))?;
        },

        "selectExportFolder" => {
            let result = nfd::open_pick_folder(None).unwrap_or_else(|e| {
                panic!(e);
            });
            match result {
                Response::Okay(file_path) => {
                    wv.eval(&format!("vue.exportfolder = {:?}", file_path))?;
                    wv.eval(&format!("vue.autoexpotfolder = false"))?;
                    wv.eval(&format!("vue.isResizeDisabled = false"))?;
                    wv.eval(&format!("vue.checkResizeDisabled()"))?;
                    
                },
                Response::Cancel => println!("User canceled"),
                _ => (),
            }
        },

        "resize" => {
            let senders = wv.user_data();

            // args :
            // 0 command, 1 filepath string, 2 source path, 3 export path, 4 target format (JPEG or PNG),
            // 5 resize mode(byW, byH, byMax, perc), 6 width, 7 height, 8 maxsize, 9 percentage,
            // 10 jpg quality, 11 bool for do not enlager
            
            // replace string to bool for donotlager
            let donotenlager:bool = match args[11] {
                "true" => true,
                "false" => false,
                _ => true
            };

            // struct paras
            let paras = ResizeParameter {
                sourcefolder: staticstr(args[2]),
                exportfolder: staticstr(args[3]),
                format: staticstr(args[4]),
                mode: staticstr(args[5]),
                width: args[6].parse::<u32>().unwrap(),
                height: args[7].parse::<u32>().unwrap(),
                maxsize: args[8].parse::<u32>().unwrap(),
                percentage: args[9].parse::<u32>().unwrap(),
                quality: args[10].parse::<i32>().unwrap(),
                donotenlager: donotenlager,
            };

            // get all files path to vector
            let files:Vec<String> = args[1].split(FILES_JOINSIGN).map(|s| s.to_string()).collect();

            // get how many threads to image resize
            let mut part = num_cpus::get();
            let mut step = files.len() / part + 1;

            // if cpus more than step, maybe error for slice like _a > files.len(), make sure part < step
            while part > step {
                part = step;
                step = files.len() / part + 1;
            };

            // works slice from all files
            let mut works_slice:Vec<Works> = vec![];
            for _i in 0..part {
                let _a = step * _i;
                let mut _b = _a + step;
                if _b > files.len() { _b = files.len() };

                let _slice = &files[_a.._b];                
                works_slice.push( Works {
                    files: _slice.to_vec(),
                    paras: paras,
                });
            }

            // senders communication
            reset_count();
            senders.tx_working.send("reset".to_string()).unwrap();
            for works in works_slice { senders.tx_launch.send(works).unwrap();}
        },

        _ => {},
    }

    Ok(())
}

// for css in tag
fn inline_style(s: &str) -> String {
    format!(r#"<style type="text/css">{}</style>"#, s)
}

// for javascript in tag
fn inline_script(s: &str) -> String {
    format!(r#"<script type="text/javascript">{}</script>"#, s)
}

// seek currect folder and return files and dirs
fn seekdir(dir:&str) -> (Vec<String>, Vec<String>) {
    let mut files:Vec<String> = Vec::new();
    let mut dirs:Vec<String> = Vec::new();
    if let Ok(entries) = fs::read_dir(dir){
        for entry in entries {
            if let Ok(entry) = entry {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_dir() {
                        dirs.push(entry.path().to_str().unwrap().to_string());
                    } else if file_type.is_file()  {
                        let s = entry.path().to_str().unwrap().to_string();
                        let mut ext:Vec<&str> = s.split(&".").collect();
                        // pick images
                        let picexts = vec!["png","PNG","jpg","JPG","jpeg","JPEG","gif","GIF"];
                        // if ext in exts
                        if picexts.contains(&ext.pop().unwrap()) { files.push(s) };
                    } else {}
                } else {}
            } else {}
        }
    }
    (files, dirs)
}

// seek sub folder and return files and a empty dirs
fn seekdir_deeper(dir:&str) -> (Vec<String>, Vec<String>){
    let mut files:Vec<String> = Vec::new();
    let mut dirs:Vec<String> = Vec::new();

    dirs.push(dir.to_string());

    while dirs.len() > 0 {
        if let Some(dir) = &dirs.pop() {
            let ret = seekdir(dir);
            files.extend(ret.0.iter().cloned());
            dirs.extend(ret.1.iter().cloned());
        }
    }

    (files, dirs)
}