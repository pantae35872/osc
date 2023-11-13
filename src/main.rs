use std::fs::{File, create_dir, remove_file, remove_dir_all};
use std::io::{BufReader, BufRead};
use std::process::{Command, Stdio};
use std::{env, fs};
use std::path::{Path, PathBuf};

fn main() {
    let args: Vec<String> = env::args().collect();
    if let Some(mode) = args.get(1) {
        if mode == "build" {
            if let Ok(current_dir) = env::current_dir() {
                let mut cargo = Command::new("cargo");
                cargo.arg("build").current_dir(current_dir.clone());
                for arg in args.iter().skip(2) {
                    cargo.arg(arg);
                }
                cargo.status().expect("Cargo build failed");
                if args.iter().skip(2).find(|&value| value == "--release").is_some() {
                    build_iso(&String::from(current_dir.join(PathBuf::from("target/config/release/nothingos")).to_str().unwrap()));
                } else {
                    build_iso(&String::from(current_dir.join(PathBuf::from("target/config/debug/nothingos")).to_str().unwrap()));
                }
            }
        }
        else if mode == "runner" {
            if let Some(path) = args.get(2) {
                let (iso, work_dir)= build_iso(path);
                if let Some(iso) = iso {
                    if let Some(work_dir) = work_dir {
                        let mut qemu = Command::new("qemu-system-x86_64");
                        qemu.arg("-cdrom").arg(iso.to_str().unwrap()).current_dir(work_dir);
                        for arg in args.iter().skip(3) {
                            qemu.arg(arg);
                        }
                        qemu.status().expect("Qemu Failed"); 
                    }
                }
            }
        }
    }
}

fn build_iso(path: &String) -> (Option<Box<Path>>, Option<Box<Path>>) {
    let mut path = path.clone();
    if !path.starts_with("/disk/data/nothingos/") { 
            path.insert_str(0, "/disk/data/nothingos/") 
        };
        let path = Path::new(&path);
        if path.parent().unwrap().file_name().unwrap() == "deps" && get_first_segment(path.file_name().unwrap().to_str().unwrap()) == "nothingos" {
            remove_file(path).unwrap();
            remove_file(path.to_str().unwrap().to_owned() + ".d").unwrap();
            return (None, None);
        }

        let mut prefix = path.file_name().unwrap().to_str().unwrap();
        if path.parent().unwrap().file_name().unwrap() != "deps" {
            prefix = get_first_segment(path.file_name().unwrap().to_str().unwrap());
        }
        if let Some(mut directory) = path.parent() {
            if path.parent().unwrap().file_name().unwrap() == "deps" {
                directory = directory.parent().unwrap(); 
            }
            let deps_dir = directory.join(PathBuf::from("deps"));
            let work_dir = directory.parent().unwrap().parent().unwrap().parent().unwrap();
            let mut index = 0;
            while {
                if directory.join(PathBuf::from("build-temp")).exists() {
                remove_dir_all(directory.join(PathBuf::from("build-temp"))).unwrap();
            }
            create_dir(directory.join(PathBuf::from("build-temp"))).unwrap();

            get_object(index, directory, path, deps_dir.clone(), prefix, work_dir);
            let object_files: Vec<String> = fs::read_dir(directory.join(PathBuf::from("build-temp")))
            .expect("Failed to read object directory")
            .filter_map(|entry| {
            if let Ok(entry) = entry {
                if let Some(extension) = entry.path().extension() {
                    if extension == "o" {
                        Some(entry.path().to_string_lossy().into_owned())
                    } else {
                        None
                    }
                    } else {
                        None
                    }
                } else {
                        None
                    }
                })
                .collect();
                let mut command = std::process::Command::new("ld");
                command.arg("-n").arg("--gc-sections").arg("-o").arg(work_dir.join(PathBuf::from("iso").join(PathBuf::from("boot").join(PathBuf::from("kernel.bin"))))).arg("-T").arg("/disk/data/nothingos/linker.ld").current_dir(directory.join(PathBuf::from("build-temp"))).stdout(Stdio::null())
                .stderr(Stdio::null());
                for object_file in &object_files {
                    command.arg(object_file);
                }
                let status = command.status().expect("Failed to run linker");
                !status.success()
            } {
                index+=1;
            }
            let mut iso_grub = Command::new("grub-mkrescue");
            iso_grub.arg("-o").arg("os.iso").arg("iso").current_dir(work_dir).stdout(Stdio::null())
                .stderr(Stdio::null());
            iso_grub.status().expect("Failed to build iso");
            return (Some(Box::from(work_dir.join(PathBuf::from("os.iso").as_path()))), Some(Box::from(work_dir)));
        }
        return (None, None);
}

fn get_files_with_extension_and_prefix(directory_path: &Path, extension: &str, prefix: &str) -> Vec<std::path::PathBuf> {
    let mut result = Vec::new();

    if let Ok(entries) = fs::read_dir(directory_path) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_file() {
                    if let Some(file_name) = entry.file_name().to_str() {
                        if file_name.starts_with(prefix) && file_name.ends_with(extension) {
                            result.push(entry.path());
                        }
                    }
                }
            }
        }
    }

    result
}

fn get_object(index: usize, directory: &Path, path: &Path, deps_dir: PathBuf, prefix: &str, work_dir: &Path) {
   get_lib(directory, path, deps_dir.clone());
   get_asm(work_dir, directory);
   get_bin(index, deps_dir, prefix, directory);
}

fn get_asm(work_dir: &Path, directory: &Path) {
    let asm_files: Vec<String> = fs::read_dir(work_dir.join(PathBuf::from("src").join(PathBuf::from("boot"))))
            .expect("Failed to read object directory")
            .filter_map(|entry| {
            if let Ok(entry) = entry {
                if let Some(extension) = entry.path().extension() {
                    if extension == "asm" {
                        Some(entry.path().to_string_lossy().into_owned())
                    } else {
                        None
                    }
                } else {
                    None
                }
                } else {
                    None
                }
            })
            .collect();
            for asm_file in &asm_files {
                let mut command = std::process::Command::new("nasm");
                command.arg("-felf64").arg(asm_file).arg("-o").arg(directory.join(PathBuf::from("build-temp")
                                                                                          .join(PathBuf::from(
                                                                                                  String::from(PathBuf::from(asm_file).file_name().unwrap()
                                                                                                               .to_str().unwrap()
                                                                                                              ).replace("asm", "o")))));
                command.status().expect("Failed to run nasm");
            }

}

fn get_lib(directory: &Path, path: &Path, deps_dir: PathBuf) {
            let d_files = get_files_with_extension_and_prefix(&deps_dir, ".d", "nothingos");
            if path.parent().unwrap().file_name().unwrap() != "deps" {
                for file in d_files {
                    let file = File::open(file).unwrap();
                    let reader = BufReader::new(file);
                
                    let lines: Vec<String> = reader.lines().map(|line| line.unwrap()).collect();
                    let mut files_i_d = Vec::new(); 
                    for line in lines.iter().rev() {
                        if line.is_empty() {
                            break;
                        }
                        files_i_d.push(line);
                    }
                    if files_i_d.iter().rev().collect::<Vec<_>>()[0].as_str() == "src/lib.rs:" {
                        let mut is_n = true;
                        for line in lines.iter() {
                            if line.starts_with("src/serial.rs:") {
                                is_n = false;
                            }
                        }
                        if is_n {
                            for line in lines.iter() {
                                if line.starts_with("src") {
                                    break;
                                }
            
                                let file = Path::new(seperate_path_makefile(line));
                                if seperate_path_makefile(line).ends_with(".a") {
                                    fs::copy(file, Path::new(&directory.join(PathBuf::from("build-temp").join(file.file_name().unwrap())))).unwrap();
                                    extract_static_library(Path::new(&directory.join(PathBuf::from("build-temp").join(file.file_name().unwrap()))).to_str().unwrap()
                                            , directory.join(PathBuf::from("build-temp")).to_str().unwrap());
                                }
                            }
                            break;
                        }
                    } 
                }
            } else {
                for file in d_files {
                    let file = File::open(file).unwrap();
                    let reader = BufReader::new(file);
                
                    let lines: Vec<String> = reader.lines().map(|line| line.unwrap()).collect();
                    let mut files_i_d = Vec::new(); 
                    for line in lines.iter().rev() {
                        if line.is_empty() {
                            break;
                        }
                        files_i_d.push(line);
                    }
                    if files_i_d.iter().rev().collect::<Vec<_>>()[0].as_str() == "src/lib.rs:" {
                        for line in lines.iter() {
                            if line.starts_with("src/serial.rs:") {
                                for line in lines.iter() {
                                    if line.starts_with("src") {
                                        break;
                                    }
            
                                    let file = Path::new(seperate_path_makefile(line));
                                    if seperate_path_makefile(line).ends_with(".a") {
                                        fs::copy(file, Path::new(&directory.join(PathBuf::from("build-temp").join(file.file_name().unwrap())))).unwrap();
                                        extract_static_library(Path::new(&directory.join(PathBuf::from("build-temp").join(file.file_name().unwrap()))).to_str().unwrap()
                                                   , directory.join(PathBuf::from("build-temp")).to_str().unwrap());
                                    }
                                }
                                break;
                            }
                        }
                    } 
                }
            }
            }
            fn get_bin(index: usize, deps_dir: PathBuf, prefix: &str, directory: &Path) {
                let d_files = get_files_with_extension_and_prefix(&deps_dir, ".d", prefix);
                let mut files: Vec<Box<Path>> = Vec::new();
                for file in d_files {
                    let file = File::open(file).unwrap();
                    let reader = BufReader::new(file);
                
                    let lines: Vec<String> = reader.lines().map(|line| line.unwrap()).collect();
                    let mut files_i_d = Vec::new(); 
                    for line in lines.iter().rev() {
                        if line.is_empty() {
                            break;
                        }
                        files_i_d.push(line);
                    }
                    if files_i_d.iter().rev().collect::<Vec<_>>()[0].as_str() != "src/lib.rs:" {
                        for line in lines.iter() {
                            if line.starts_with("src") {
                                break;
                            }
                            let file = Path::new(seperate_path_makefile(line));
                            if seperate_path_makefile(line).ends_with(".o") {
                                files.push(Box::from(file));
                            }
                        }
                    }
                }
                if let Some(file) = files.get(index) {
                    fs::copy(file, Path::new(&directory.join(PathBuf::from("build-temp").join(file.file_name().unwrap())))).unwrap();
                }
            }


fn extract_static_library(library_path: &str, output_directory: &str) {
    // Create the output directory if it doesn't exist
    std::fs::create_dir_all(output_directory).expect("Failed to create output directory");

    // Use the ar command to extract the .a file into the output directory
    Command::new("ar")
        .args(&["x", library_path])
        .current_dir(output_directory)
        .status()
        .expect("Failed to execute ar command");
}

fn get_first_segment(input: &str) -> &str {
    if let Some(index) = input.rfind('-') {
        &input[0..index]
    } else {
        input
    }
}

fn seperate_path_makefile(input: &str) -> &str {
    if let Some(index) = input.rfind(':') {
        &input[0..index]
    } else {
        input
    }
}
