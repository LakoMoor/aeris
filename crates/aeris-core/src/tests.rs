use super::*;

fn s(f: Flavor) -> Session {
    Session::new(f, Stamp::new(2026, 11, 4, 13, 51), f == Flavor::Mac)
}

#[test]
fn prompts_match_each_dialect() {
    assert_eq!(s(Flavor::Cmd).prompt(), "C:\\Users\\aeris>");
    assert_eq!(s(Flavor::PowerShell).prompt(), "PS C:\\Users\\aeris> ");
    assert_eq!(s(Flavor::Mac).prompt(), "aeris@aeris-mac ~ % ");
}

#[test]
fn cmd_dir_and_cd() {
    let mut x = s(Flavor::Cmd);
    let o = x.exec("dir");
    assert!(o.out.contains("Directory of C:\\Users\\aeris"));
    assert!(o.out.contains("<DIR>          Documents"));
    x.exec("cd Documents");
    assert_eq!(x.prompt(), "C:\\Users\\aeris\\Documents>");
    let o = x.exec("type readme.txt");
    assert!(o.out.contains("Welcome to Aeris OS!"));
    x.exec("cd ..\\..");
    assert_eq!(x.cwd, "/Users");
    let o = x.exec("cd nowhere");
    assert_eq!(o.err.trim(), "The system cannot find the path specified.");
}

#[test]
fn powershell_listing_and_aliases() {
    let mut x = s(Flavor::PowerShell);
    let o = x.exec("Get-ChildItem");
    assert!(o.out.contains("Mode                 LastWriteTime         Length Name"));
    assert!(o.out.contains("d-----"));
    let o2 = x.exec("ls");
    assert_eq!(o.out, o2.out);
    x.exec("New-Item -ItemType Directory -Path demo");
    assert!(x.vfs.is_dir("/Users/aeris/demo"));
    x.exec("Set-Content -Path demo/a.txt -Value hello");
    assert_eq!(x.exec("Get-Content demo/a.txt").out.trim(), "hello");
    let o = x.exec("Remove-Item demo");
    assert!(o.err.contains("Recurse"));
    x.exec("Remove-Item demo -Recurse");
    assert!(!x.vfs.exists("/Users/aeris/demo"));
}

#[test]
fn powershell_variables_and_pipes() {
    let mut x = s(Flavor::PowerShell);
    x.exec("$name = 'Aeris'");
    assert_eq!(x.exec("echo \"hi $name\"").out.trim(), "hi Aeris");
    let o = x.exec("cat Documents/todo.txt | Select-String Rust");
    assert!(o.out.contains("write more Rust"));
    assert_eq!(x.exec("$env:USERNAME").err.is_empty(), false); // bare expression is not a command
}

#[test]
fn mac_terminal() {
    let mut x = s(Flavor::Mac);
    let o = x.exec("ls");
    assert!(o.out.contains("Documents"));
    x.exec("mkdir -p a/b/c");
    assert!(x.vfs.is_dir("/Users/aeris/a/b/c"));
    let o = x.exec("rm a");
    assert!(o.err.contains("is a directory"));
    x.exec("rm -rf a");
    assert!(!x.vfs.exists("/Users/aeris/a"));
    x.exec("echo hi > t.txt");
    assert_eq!(x.exec("cat t.txt").out, "hi\n");
    x.exec("echo more >> t.txt");
    assert_eq!(x.exec("cat t.txt | wc -l").out.trim(), "2");
    assert_eq!(x.exec("cd /nope").err.trim(), "cd: /nope: No such file or directory");
    assert_eq!(x.exec("nothing").err.trim(), "zsh: command not found: nothing");
    x.exec("export FOO=bar");
    assert_eq!(x.exec("echo $FOO").out, "bar\n");
}

#[test]
fn shell_switching_returns_on_exit() {
    let mut x = s(Flavor::Cmd);
    let o = x.exec("powershell");
    assert!(o.switched);
    assert_eq!(x.flavor, Flavor::PowerShell);
    let o = x.exec("exit");
    assert!(!o.exit);
    assert_eq!(x.flavor, Flavor::Cmd);
    assert!(x.exec("exit").exit);
}

#[test]
fn tab_completion() {
    let x = s(Flavor::PowerShell);
    assert_eq!(x.complete("cd Doc"), alloc::vec!["Documents\\".to_string()]);
    assert!(x.complete("Get-Ch").contains(&"Get-ChildItem".to_string()));
}

#[test]
fn vfs_roundtrips_through_bytes() {
    let mut x = s(Flavor::PowerShell);
    x.exec("mkdir proj");
    x.exec("Set-Content -Path proj/a.txt -Value 'privet, mir'");
    let bytes = x.vfs.to_bytes();
    let y = vfs::Vfs::from_bytes(&bytes, Stamp::new(2026, 11, 5, 0, 0)).expect("decode");
    assert_eq!(y.read("/Users/aeris/proj/a.txt").unwrap(), "privet, mir\n");
    assert!(y.is_dir("/Users/aeris/Documents"));
    assert!(vfs::Vfs::from_bytes(&bytes[..bytes.len() - 3], Stamp::default()).is_none());
    assert!(vfs::Vfs::from_bytes(b"junk", Stamp::default()).is_none());
}

#[test]
fn weekday_is_right() {
    // 2026-11-04 is a Wednesday.
    assert_eq!(Stamp::new(2026, 11, 4, 0, 0).weekday(), 3);
}

#[test]
fn fetch_shows_resolution() {
    let mut x = s(Flavor::Mac);
    x.sys.width = 1920;
    x.sys.height = 1080;
    assert!(x.exec("neofetch").out.contains("1920x1080"));
}
