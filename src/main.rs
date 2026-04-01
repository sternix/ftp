mod ftp;

use ftp::{FtpClient, TransferType};
use std::fs;
use std::io::{self, Write};
use std::path::Path;

fn print_help() {
    println!(
        "
Kullanılabilir komutlar:
  open <host> [port]     - FTP sunucusuna bağlan (varsayılan port: 21)
  user <kullanıcı> <şifre> - Giriş yap
  ls [yol]               - Dosyaları listele
  dir [yol]              - Dosyaları listele (detaylı)
  pwd                    - Mevcut dizini göster
  cd <yol>               - Dizin değiştir
  cdup                   - Üst dizine geç
  get <uzak> [yerel]     - Dosya indir
  put <yerel> [uzak]     - Dosya yükle
  delete <dosya>         - Dosya sil
  mkdir <dizin>          - Dizin oluştur
  rmdir <dizin>          - Dizin sil
  rename <eski> <yeni>   - Dosya/dizin adını değiştir
  size <dosya>           - Dosya boyutunu göster
  ascii                  - ASCII transfer moduna geç
  binary                 - Binary transfer moduna geç
  syst                   - Sunucu sistem bilgisi
  stat [yol]             - Sunucu/dosya durum bilgisi
  noop                   - Bağlantıyı canlı tut
  quit / exit            - Çıkış
  help                   - Bu yardım mesajı
"
    );
}

fn prompt(connected: bool) -> String {
    if connected {
        "ftp> ".to_string()
    } else {
        "ftp (bağlı değil)> ".to_string()
    }
}

fn main() {
    println!("Rust FTP İstemcisi v0.1.0");
    println!("Yardım için 'help' yazın.\n");

    let mut client: Option<FtpClient> = None;
    let stdin = io::stdin();

    loop {
        let p = prompt(client.is_some());
        print!("{}", p);
        io::stdout().flush().unwrap();

        let mut input = String::new();
        if stdin.read_line(&mut input).unwrap() == 0 {
            break;
        }

        let input = input.trim();
        if input.is_empty() {
            continue;
        }

        let parts: Vec<&str> = input.splitn(3, ' ').collect();
        let cmd = parts[0].to_lowercase();

        match cmd.as_str() {
            "help" | "?" => print_help(),

            "open" | "connect" => {
                if client.is_some() {
                    println!("Zaten bağlı. Önce 'quit' ile bağlantıyı kapatın.");
                    continue;
                }
                if parts.len() < 2 {
                    println!("Kullanım: open <host> [port]");
                    continue;
                }
                let host = parts[1];
                let port = if parts.len() > 2 {
                    parts[2].parse::<u16>().unwrap_or(21)
                } else {
                    21
                };
                let addr = format!("{}:{}", host, port);
                println!("{}:{} adresine bağlanılıyor...", host, port);
                match FtpClient::connect(&addr) {
                    Ok((c, resp)) => {
                        println!("{}", resp.message);
                        client = Some(c);
                    }
                    Err(e) => println!("Bağlantı hatası: {}", e),
                }
            }

            "user" | "login" => {
                let c = match client.as_mut() {
                    Some(c) => c,
                    None => {
                        println!("Önce bir sunucuya bağlanın (open komutu).");
                        continue;
                    }
                };
                if parts.len() < 3 {
                    println!("Kullanım: user <kullanıcı> <şifre>");
                    continue;
                }
                let user = parts[1];
                let pass = parts[2];
                match c.login(user, pass) {
                    Ok(resp) => println!("{}", resp.message),
                    Err(e) => println!("Hata: {}", e),
                }
            }

            "ls" | "nlst" => {
                let c = match client.as_mut() {
                    Some(c) => c,
                    None => {
                        println!("Önce bir sunucuya bağlanın.");
                        continue;
                    }
                };
                let path = parts.get(1).copied();
                match c.nlst(path) {
                    Ok((resp, data)) => {
                        if !data.is_empty() {
                            print!("{}", data);
                        }
                        if !resp.is_success() {
                            println!("{}", resp.message);
                        }
                    }
                    Err(e) => println!("Hata: {}", e),
                }
            }

            "dir" | "list" => {
                let c = match client.as_mut() {
                    Some(c) => c,
                    None => {
                        println!("Önce bir sunucuya bağlanın.");
                        continue;
                    }
                };
                let path = parts.get(1).copied();
                match c.list(path) {
                    Ok((resp, data)) => {
                        if !data.is_empty() {
                            print!("{}", data);
                        }
                        if !resp.is_success() {
                            println!("{}", resp.message);
                        }
                    }
                    Err(e) => println!("Hata: {}", e),
                }
            }

            "pwd" => {
                let c = match client.as_mut() {
                    Some(c) => c,
                    None => {
                        println!("Önce bir sunucuya bağlanın.");
                        continue;
                    }
                };
                match c.pwd() {
                    Ok(resp) => println!("{}", resp.message),
                    Err(e) => println!("Hata: {}", e),
                }
            }

            "cd" | "cwd" => {
                let c = match client.as_mut() {
                    Some(c) => c,
                    None => {
                        println!("Önce bir sunucuya bağlanın.");
                        continue;
                    }
                };
                if parts.len() < 2 {
                    println!("Kullanım: cd <yol>");
                    continue;
                }
                match c.cwd(parts[1]) {
                    Ok(resp) => println!("{}", resp.message),
                    Err(e) => println!("Hata: {}", e),
                }
            }

            "cdup" => {
                let c = match client.as_mut() {
                    Some(c) => c,
                    None => {
                        println!("Önce bir sunucuya bağlanın.");
                        continue;
                    }
                };
                match c.cdup() {
                    Ok(resp) => println!("{}", resp.message),
                    Err(e) => println!("Hata: {}", e),
                }
            }

            "get" | "recv" | "download" => {
                let c = match client.as_mut() {
                    Some(c) => c,
                    None => {
                        println!("Önce bir sunucuya bağlanın.");
                        continue;
                    }
                };
                if parts.len() < 2 {
                    println!("Kullanım: get <uzak_dosya> [yerel_dosya]");
                    continue;
                }
                let remote = parts[1];
                let local = if parts.len() > 2 {
                    parts[2].to_string()
                } else {
                    Path::new(remote)
                        .file_name()
                        .map(|f| f.to_string_lossy().to_string())
                        .unwrap_or_else(|| remote.to_string())
                };

                println!("İndiriliyor: {} -> {}", remote, local);
                match c.download(remote) {
                    Ok((resp, data)) => {
                        if resp.is_success() && !data.is_empty() {
                            match fs::write(&local, &data) {
                                Ok(_) => println!(
                                    "Başarılı. {} byte indirildi.",
                                    data.len()
                                ),
                                Err(e) => println!("Dosya yazma hatası: {}", e),
                            }
                        } else if !resp.is_success() {
                            println!("{}", resp.message);
                        }
                    }
                    Err(e) => println!("Hata: {}", e),
                }
            }

            "put" | "send" | "upload" => {
                let c = match client.as_mut() {
                    Some(c) => c,
                    None => {
                        println!("Önce bir sunucuya bağlanın.");
                        continue;
                    }
                };
                if parts.len() < 2 {
                    println!("Kullanım: put <yerel_dosya> [uzak_dosya]");
                    continue;
                }
                let local = parts[1];
                let remote = if parts.len() > 2 {
                    parts[2].to_string()
                } else {
                    Path::new(local)
                        .file_name()
                        .map(|f| f.to_string_lossy().to_string())
                        .unwrap_or_else(|| local.to_string())
                };

                match fs::read(local) {
                    Ok(data) => {
                        println!("Yükleniyor: {} -> {} ({} byte)", local, remote, data.len());
                        match c.upload(&remote, &data) {
                            Ok(resp) => println!("{}", resp.message),
                            Err(e) => println!("Hata: {}", e),
                        }
                    }
                    Err(e) => println!("Yerel dosya okuma hatası: {}", e),
                }
            }

            "delete" | "del" | "rm" => {
                let c = match client.as_mut() {
                    Some(c) => c,
                    None => {
                        println!("Önce bir sunucuya bağlanın.");
                        continue;
                    }
                };
                if parts.len() < 2 {
                    println!("Kullanım: delete <dosya>");
                    continue;
                }
                match c.delete(parts[1]) {
                    Ok(resp) => println!("{}", resp.message),
                    Err(e) => println!("Hata: {}", e),
                }
            }

            "mkdir" => {
                let c = match client.as_mut() {
                    Some(c) => c,
                    None => {
                        println!("Önce bir sunucuya bağlanın.");
                        continue;
                    }
                };
                if parts.len() < 2 {
                    println!("Kullanım: mkdir <dizin>");
                    continue;
                }
                match c.mkdir(parts[1]) {
                    Ok(resp) => println!("{}", resp.message),
                    Err(e) => println!("Hata: {}", e),
                }
            }

            "rmdir" => {
                let c = match client.as_mut() {
                    Some(c) => c,
                    None => {
                        println!("Önce bir sunucuya bağlanın.");
                        continue;
                    }
                };
                if parts.len() < 2 {
                    println!("Kullanım: rmdir <dizin>");
                    continue;
                }
                match c.rmdir(parts[1]) {
                    Ok(resp) => println!("{}", resp.message),
                    Err(e) => println!("Hata: {}", e),
                }
            }

            "rename" | "mv" => {
                let c = match client.as_mut() {
                    Some(c) => c,
                    None => {
                        println!("Önce bir sunucuya bağlanın.");
                        continue;
                    }
                };
                if parts.len() < 3 {
                    println!("Kullanım: rename <eski_ad> <yeni_ad>");
                    continue;
                }
                match c.rename(parts[1], parts[2]) {
                    Ok(resp) => println!("{}", resp.message),
                    Err(e) => println!("Hata: {}", e),
                }
            }

            "size" => {
                let c = match client.as_mut() {
                    Some(c) => c,
                    None => {
                        println!("Önce bir sunucuya bağlanın.");
                        continue;
                    }
                };
                if parts.len() < 2 {
                    println!("Kullanım: size <dosya>");
                    continue;
                }
                match c.size(parts[1]) {
                    Ok(resp) => println!("{}", resp.message),
                    Err(e) => println!("Hata: {}", e),
                }
            }

            "ascii" => {
                let c = match client.as_mut() {
                    Some(c) => c,
                    None => {
                        println!("Önce bir sunucuya bağlanın.");
                        continue;
                    }
                };
                match c.set_type(TransferType::Ascii) {
                    Ok(resp) => println!("{}", resp.message),
                    Err(e) => println!("Hata: {}", e),
                }
            }

            "binary" | "bin" => {
                let c = match client.as_mut() {
                    Some(c) => c,
                    None => {
                        println!("Önce bir sunucuya bağlanın.");
                        continue;
                    }
                };
                match c.set_type(TransferType::Binary) {
                    Ok(resp) => println!("{}", resp.message),
                    Err(e) => println!("Hata: {}", e),
                }
            }

            "syst" => {
                let c = match client.as_mut() {
                    Some(c) => c,
                    None => {
                        println!("Önce bir sunucuya bağlanın.");
                        continue;
                    }
                };
                match c.syst() {
                    Ok(resp) => println!("{}", resp.message),
                    Err(e) => println!("Hata: {}", e),
                }
            }

            "stat" | "status" => {
                let c = match client.as_mut() {
                    Some(c) => c,
                    None => {
                        println!("Önce bir sunucuya bağlanın.");
                        continue;
                    }
                };
                let path = parts.get(1).copied();
                match c.stat(path) {
                    Ok(resp) => println!("{}", resp.message),
                    Err(e) => println!("Hata: {}", e),
                }
            }

            "noop" | "ping" => {
                let c = match client.as_mut() {
                    Some(c) => c,
                    None => {
                        println!("Önce bir sunucuya bağlanın.");
                        continue;
                    }
                };
                match c.noop() {
                    Ok(resp) => println!("{}", resp.message),
                    Err(e) => println!("Hata: {}", e),
                }
            }

            "quit" | "exit" | "bye" => {
                if let Some(ref mut c) = client {
                    let _ = c.quit();
                }
                println!("Güle güle!");
                break;
            }

            _ => {
                println!("Bilinmeyen komut: '{}'. Yardım için 'help' yazın.", cmd);
            }
        }
    }
}
