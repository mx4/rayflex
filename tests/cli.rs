use assert_cmd::prelude::*;
use std::process::Command;

#[test]
fn scene_teapot() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("rayflex")?;
    cmd.arg("-l").arg("scenes/teapot.json").assert().success();

    Ok(())
}
#[test]
fn scene_trolley() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("rayflex")?;
    cmd.arg("-l").arg("scenes/trolley.json").assert().success();

    Ok(())
}
#[test]
fn scene_buddha() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("rayflex")?;
    cmd.arg("-l").arg("scenes/buddha.json").assert().success();

    Ok(())
}
#[test]
fn scene_cow() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("rayflex")?;
    cmd.arg("-l").arg("scenes/cow.json").assert().success();

    Ok(())
}
#[test]
fn scene_gold_gallery() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("rayflex")?;
    cmd.arg("-l")
        .arg("scenes/gold-gallery.json")
        .arg("-x")
        .arg("300")
        .arg("-y")
        .arg("192")
        .arg("-p")
        .arg("5")
        .assert()
        .success();

    Ok(())
}
#[test]
fn scene_cornell_box() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("rayflex")?;
    cmd.arg("-l")
        .arg("scenes/cornell-box.json")
        .arg("-p")
        .arg("5")
        .assert()
        .success();

    Ok(())
}
#[test]
fn scene_suzanne_bust() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("rayflex")?;
    cmd.arg("-l")
        .arg("scenes/suzanne-bust.json")
        .arg("-x")
        .arg("300")
        .arg("-y")
        .arg("300")
        .arg("-p")
        .arg("5")
        .assert()
        .success();

    Ok(())
}
#[test]
fn scene_torus_knot() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("rayflex")?;
    cmd.arg("-l")
        .arg("scenes/torus-knot.json")
        .arg("-x")
        .arg("300")
        .arg("-y")
        .arg("300")
        .arg("-p")
        .arg("5")
        .assert()
        .success();

    Ok(())
}
