extern crate image;

use libc;
use mozjpeg_sys::*;
use std::mem;
use std::ffi::CString;
use rand::{thread_rng, Rng};
use rand::distributions::Alphanumeric;

use std::time::Instant;
use std::{
    env,
    fs,
    fs::{File,},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    sync::mpsc,
};

use async_std::task;

// signs for communication of webview and rust
pub const CMDS_JOINSIGN:&str = "🀡🀡";
pub const FILES_JOINSIGN:&str = "🀅🀅";

// struct for resize parameters
#[derive(Copy, Clone)]
pub struct ResizeParameter {
    pub sourcefolder:&'static str,
    pub exportfolder:&'static str, 
    pub format:&'static str, 
    pub mode:&'static str,
    pub width:u32, 
    pub height:u32, 
    pub maxsize:u32,
    pub percentage:u32,
    pub quality:i32,
    pub donotenlager: bool,
}

// struct for works files and parameters
pub struct Works {
    pub files: Vec<String>,
    pub paras: ResizeParameter,
}

// struct for inter thread communication
pub struct Senders{
    pub tx_launch: mpsc::Sender<Works>,
    pub tx_working: mpsc::Sender<String>,
}

// for mozjpeg save tempfiles with random file name
fn randstring() -> String {
    let rand_string: String = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(30)
        .collect();

    rand_string
}

// for block_on use &str args
// ref: https://stackoverflow.com/questions/23975391/how-to-convert-a-string-into-a-static-str
pub fn staticstr(_s:&str) -> &'static str{
    let __s: &'static str = Box::leak(_s.to_string().into_boxed_str());
    __s
}

// mozjpeg for compress jpegs
// creat image-rs save jpegs directly that file size is more big
unsafe fn encode(buffer: &[u8], width: u32, height: u32, quality: i32, imgpath: String) -> std::io::Result<()>{
    // libc not work well for utf8, so use tempfile within safe filename in system temp dir
    let mut tempfile = env::temp_dir();
    tempfile.push(format!(".temp_{}", randstring()));
    // println!("{:?}", &tempfile);

    let quality = quality;
    let file_name = CString::new(tempfile.to_str().unwrap()).unwrap();
    
    let mode = CString::new("wb").unwrap();
    let fh = libc::fopen(file_name.as_ptr(), mode.as_ptr());

    let mut err = mem::zeroed();
    let mut cinfo: jpeg_compress_struct = mem::zeroed();
    cinfo.common.err = jpeg_std_error(&mut err);
    jpeg_create_compress(&mut cinfo);
    jpeg_stdio_dest(&mut cinfo, fh);

    cinfo.image_width = width;
    cinfo.image_height = height;
    cinfo.in_color_space = J_COLOR_SPACE::JCS_RGB;
    cinfo.input_components = 3;
    jpeg_set_defaults(&mut cinfo);

    let row_stride = cinfo.image_width as usize * cinfo.input_components as usize;
    cinfo.dct_method = J_DCT_METHOD::JDCT_ISLOW;
    // units = 0: no units, X and Y specify the pixel
    // aspect ratio
    // units = 1: X and Y are dots per inch
    // units = 2: X and Y are dots per cm 
    // ref: https://www.w3.org/Graphics/JPEG/jfif3.pdf
    cinfo.density_unit = 1;
    cinfo.X_density = 72;
    cinfo.Y_density = 72;
    jpeg_set_quality(&mut cinfo, quality, true as boolean);

    jpeg_start_compress(&mut cinfo, true as boolean);

    while cinfo.next_scanline < cinfo.image_height {
        let offset = cinfo.next_scanline as usize * row_stride;
        let jsamparray = [buffer[offset..].as_ptr()];
        jpeg_write_scanlines(&mut cinfo, jsamparray.as_ptr(), 1);
    }

    jpeg_finish_compress(&mut cinfo);
    jpeg_destroy_compress(&mut cinfo);
    libc::fclose(fh);

    match fs::copy(&tempfile, &imgpath){
        Ok(_) => {},
        Err(e) => println!("copy_file Err {:?}, {:?},({:?})", e, imgpath, tempfile),
    }
    
    match fs::remove_file(&tempfile) {
        Ok(_) => {},
        Err(e) => println!("remove_file Err {:?}, {:?},({:?})", e, imgpath, tempfile),
    }

    Ok(())
}

async fn loadimg(i:&str) -> image::ImageResult<image::DynamicImage> {
    match image::open(&i) {
        Ok(img) => Ok(img),
        Err(e) => Err(e),
    }
}

async fn resizeimg(img:image::DynamicImage, _i:&str, para:ResizeParameter) -> image::DynamicImage{
    match para.mode {
        "ByWidth" => {
            let img_buf = &img.to_rgba();
            if para.donotenlager && &para.width >= &img_buf.width() {
                img    
            }else{
                let new_img = img.resize(para.width, 1000000, image::imageops::FilterType::CatmullRom);
                new_img
            }
        },
        "ByHeight" => {
            let img_buf = &img.to_rgba();
            if para.donotenlager && &para.height >= &img_buf.height() {
                img
            }else{
                let new_img = img.resize(1000000, para.height, image::imageops::FilterType::CatmullRom);
                new_img
            }
        },
        "ByMaxSize" => {
            let img_buf = &img.to_rgba();
            if para.donotenlager && &img_buf.height() >= &para.height {
                img
            }else{
                let new_img = img.resize(para.maxsize, para.maxsize, image::imageops::FilterType::CatmullRom);
                new_img
            }
        },
        "Percentage" => {
            let img_buf = &img.to_rgba();
            let new_width = &img_buf.width() * para.percentage / 100;
            let new_height = &img_buf.height() * para.percentage / 100;
            let new_img = img.resize(new_width, new_height, image::imageops::FilterType::CatmullRom);
            new_img
        },
        _ => unimplemented!()
    }
}

async fn saveimg(img: image::DynamicImage, i:&str, para:ResizeParameter) -> Result<(), &str>{
    let path:String;

    if para.sourcefolder != "" {
        // if source folder, replace with export folder
        // the export will keep sub folder struct save as source folder
        let _i = &i.replace(para.sourcefolder, para.exportfolder);

        let _path = Path::new(_i);
        let _parent = _path.parent().unwrap();
        let _file_stem = _path.file_stem().unwrap();
        let _extension = para.format;
        
        let mut pathbuf = PathBuf::new();
        pathbuf.push(_parent);
        pathbuf.push(_file_stem);
        pathbuf.set_extension(_extension);

        path = pathbuf.as_path().to_string_lossy().to_string();
    } else {
        // if no source folder, place the file to export folder
        let _path = Path::new(i);
        let _parent = para.exportfolder;
        let _file_stem = _path.file_stem().unwrap();
        let _extension = para.format;

        let mut pathbuf = PathBuf::new();
        pathbuf.push(_parent);
        pathbuf.push(_file_stem);
        pathbuf.set_extension(_extension);

        path = pathbuf.as_path().to_string_lossy().to_string();
    }
    
    // create if export folder not exist
    let path = Path::new(&path);
    let dir = path.parent().unwrap();
    if !dir.exists() {
        match fs::create_dir_all(dir){
            Ok(_) => {},
            Err(e) => {println!("create_dir Err {:?}, {:?}", e, dir)},
        };
    }
    
    
    // write with target format
    match para.format {
        "jpg" => {
            let img_buf = img.to_rgb();
            // let mut writer = BufWriter::new(File::create(&format!("{}", path.to_string_lossy())).unwrap());
            // let mut encoder = image::jpeg::JPEGEncoder::new(&mut writer);
            // encoder.set_pixel_density(image::jpeg::PixelDensity::dpi(72));
            // encoder.encode(&img_buf, img_buf.width(), img_buf.height(), image::ColorType::Rgb8).unwrap();
            // img.write_to(&mut writer, image::ImageOutputFormat::Jpeg(para.quality)).unwrap();
            unsafe {
                let _ = encode(&img_buf, img_buf.width(), img_buf.height(), para.quality, path.to_string_lossy().to_string());    
            };
        },

        "png" => {
            let img_buf = img.to_rgba();
            let mut writer = BufWriter::new(File::create(&format!("{}", path.to_string_lossy())).unwrap());
            let encoder = image::png::PNGEncoder::new(&mut writer);
            encoder.encode(&img_buf, img_buf.width(), img_buf.height(), image::ColorType::Rgba8).unwrap();
            img.write_to(&mut writer, image::ImageOutputFormat::Png).unwrap();
            writer.flush().unwrap();
        },

        _ => {}
    }

    Ok(())
}


async fn proc(i:&str, _imgid:u32, para:ResizeParameter, tx:mpsc::Sender<String>){
    // sender 放在这里，才能及时发信号回接收器，放在 tasks_main 中的 task::block_on 里，是全部执行完才返回信号。
    // tx.send 回传 imgid 进度
    let _f = Path::new(i).file_name().unwrap();
    let _f_name = _f.to_str().unwrap();
    let _start = Instant::now();
    // println!("start {:?}: {:?}", &imgid, _start.elapsed());
    match loadimg(i).await {
        Ok(img) => {
            // println!("loaded {:?}: {:?}", &_imgid, _start.elapsed());
            let img = resizeimg(img, &i, para).await;
            // println!("resized {:?}: {:?}", &_imgid, _start.elapsed());
            match saveimg(img, i, para).await {
                Ok(_) => {
                    // println!("saved {:?}: {:?}", &_imgid, _start.elapsed());
                    let _count = count("COUNT");
                    let _s = vec!["working", &_count.to_string(), _f_name].join(CMDS_JOINSIGN);
                    tx.send(_s.to_string()).unwrap();
                },
                Err(_) => {
                    // tx.send 回传错误
                    let _fails = count("FAILS");
                    let _s = vec!["err", &_fails.to_string(), i, "save failure."].join(CMDS_JOINSIGN);
                    tx.send(_s.to_string()).unwrap();
                    
                    // tx.send 回传进度
                    let _count = count("GET");
                    let _s = vec!["working", &_count.to_string(), _f_name].join(CMDS_JOINSIGN);
                    tx.send(_s.to_string()).unwrap();
                }
            };
        },
        Err(e) => {
            // tx.send 回传错误
            let __s = format!("{}", e);
            let _fails = count("FAILS");
            let _s = vec!["err", &_fails.to_string(), i, &__s].join(CMDS_JOINSIGN);
            tx.send(_s.to_string()).unwrap();

            // tx.send 回传进度
            let _count = count("GET");
            let _s = vec!["working", &_count.to_string(), _f_name].join(CMDS_JOINSIGN);
            tx.send(_s.to_string()).unwrap();
        }
    }
}

// use unsafe for processing count
// maybe lose count if senders send to main thread too many times in a moment
// so count in unsafe static variable, and send to main thread one time one moment
// maybe mutex?
fn count(cmd:&str) -> u32 {
    static mut COUNT: u32 = 0;
    static mut FAILS: u32 = 0;
    unsafe {
        match cmd {
            "RESET" => {
                COUNT = 0;
                FAILS = 0;
                0
            },
            "COUNT" => {
                COUNT += 1;
                COUNT
            },
            "FAILS" => {
                FAILS += 1;
                FAILS
            },
            "GET" => {
                COUNT
            },
            _ => COUNT
        }
    }
}

pub fn get_count() -> u32 {
    count("GET")
}

pub fn reset_count() -> u32 {
    count("RESET")
}

pub fn tasks_main(v:Vec<String>, para:ResizeParameter, tx:mpsc::Sender<String>) {
    let mut images = Vec::new();
    images.extend(v.iter().cloned());
    
    task::block_on(async{
        let start = Instant::now();
        let mut tasks = Vec::new();

        let mut imgid:u32 = 1;
        for i in images {
            let _tx = tx.clone();
            tasks.push(task::spawn(async move{
                proc(i.as_str(), imgid, para, _tx).await;
            }));
            imgid += 1;
        }

        for t in tasks {
            t.await;
        }

        dbg!(start.elapsed());
        let __s = &format!("{:?}", start.elapsed());
        let _s = vec!["done", __s].join(CMDS_JOINSIGN);
        tx.send(_s.to_string()).unwrap();
    });
}