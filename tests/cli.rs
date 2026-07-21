use assert_cmd::prelude::*;
use std::process::Command;

#[test]
fn scene_glass_ball() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("rayflex")?;
    cmd.arg("-l")
        .arg("scenes/glass-ball.json")
        .arg("-x")
        .arg("200")
        .arg("-y")
        .arg("200")
        .arg("--img-file")
        .arg("/tmp/rayflex-test-glass-ball.png")
        .assert()
        .success();

    Ok(())
}
#[test]
fn scene_glass_ball_path_traced() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("rayflex")?;
    cmd.arg("-l")
        .arg("scenes/glass-ball.json")
        .arg("-x")
        .arg("200")
        .arg("-y")
        .arg("200")
        .arg("-p")
        .arg("10")
        .arg("--img-file")
        .arg("/tmp/rayflex-test-glass-ball-pt.png")
        .assert()
        .success();

    Ok(())
}
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
#[test]
fn scene_toybox() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("rayflex")?;
    cmd.arg("-l")
        .arg("scenes/toybox.json")
        .arg("-x")
        .arg("240")
        .arg("-y")
        .arg("180")
        .arg("-p")
        .arg("4")
        .assert()
        .success();

    Ok(())
}
#[test]
fn scene_sponza() -> Result<(), Box<dyn std::error::Error>> {
    // Tiny render on purpose: this scene is dominated by its fixed load cost
    // (23 MB OBJ + 21 textures + one BVH over 227k triangles, ~1.8s), so the
    // pixels are nearly free and the test still covers the whole path.
    let mut cmd = Command::cargo_bin("rayflex")?;
    cmd.arg("-l")
        .arg("scenes/sponza.json")
        .arg("-x")
        .arg("160")
        .arg("-y")
        .arg("100")
        .arg("-p")
        .arg("2")
        .assert()
        .success();

    Ok(())
}
