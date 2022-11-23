use assert_cmd::prelude::*;
use std::process::Command;

#[test]
fn scene_teapot() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("raymax")?;
    cmd.arg("-l").arg("scenes/teapot.json").assert().success();

    Ok(())
}
#[test]
fn scene_trolley() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("raymax")?;
    cmd.arg("-l").arg("scenes/trolley.json").assert().success();

    Ok(())
}
#[test]
fn scene_buddha() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("raymax")?;
    cmd.arg("-l").arg("scenes/buddha.json").assert().success();

    Ok(())
}
#[test]
fn scene_cow() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("raymax")?;
    cmd.arg("-l").arg("scenes/cow.json").assert().success();

    Ok(())
}
#[test]
fn scene_sphere_box() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("raymax")?;
    cmd.arg("-l").arg("scenes/sphere-box.json").assert().success();

    Ok(())
}
#[test]
fn scene_sphere_no_box() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("raymax")?;
    cmd.arg("-l").arg("scenes/sphere-nobox.json").assert().success();

    Ok(())
}
