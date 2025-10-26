# miai

[![Crates.io Version](https://img.shields.io/crates/v/miai)](https://crates.io/crates/miai)
[![docs.rs](https://img.shields.io/docsrs/miai)](https://docs.rs/miai)
[![Crates.io License](https://img.shields.io/crates/l/miai)](/LICENSE)

调用你的小米、小爱音箱，或其他任何支持的小爱设备。

灵感和实现思路源于 [miservice_fork](https://github.com/yihong0618/MiService)，但主要聚焦于小爱音箱这一设备。

## 主要功能

- 播报文本。
- 播放音乐。
- 调整音量。
- 控制播放状态。
- 执行文本（询问小爱）。
- 提供底层接口，或许能帮助你发现更多！

## 命令行工具

提供一个简单的命令行工具 `xiaoai`，可以从命令行操作小爱。

- 登录

  ```sh
  xiaoai login
  ```

- 列出设备

  ```sh
  xiaoai device
  ```

- 询问小爱

  ```sh
  xiaoai ask '今天天气怎么样'
  ```

- 播报文本

  ```sh
  xiaoai say '今天天气挺好的'
  ```

- 播放音乐

  ```sh
  xiaoai play 'http://music-url'
  ```

- 调整音量

  ```sh
  xiaoai volume 66
  ```

- 播放控制

  ```sh
  xiaoai play  # 播放
  xiaoai pause  # 暂停
  xiaoai stop   # 停止
  ```

- 认证均使用认证文件，可以指定认证文件的路径

  ```sh
  # 认证文件默认使用当前目录的 xiaoai-auth.json
  # 相当于 xiaoai --auth-file xiaoai-auth.json device
  xiaoai device

  # 登录可以获得认证文件
  xiaoai --auth-file my-auth.json login

  # 其他命令使用认证文件进行认证
  xiaoai --auth-file my-auth.json device
  ```

- 如果你知道一个设备的 ID，也可以在命令行指定

  ```sh
  # 不指定的话会看情况选择设备
  xiaoai --device-id <DEVICE_ID> play
  ```

## 在项目中使用

`miai` 提供了一组简单的 API 帮助调用小爱，要用于 Rust 项目，只需要添加依赖：

```sh
cargo add miai
```

使用示例：

```rust
use miai::{PlayState, Xiaoai};

#[tokio::main]
async fn main() {
    // 登录你的账号
    let xiaoai = Xiaoai::login("username", "password").await.unwrap();

    // 查询你的设备信息
    let device_info = xiaoai.device_info().await.unwrap();

    for info in device_info {
        // device_id 为请求指明目标设备
        let device_id = info.device_id;

        // 让设备播报文本
        xiaoai.tts(&device_id, "你好！").await.unwrap();

        // 提供一个链接，让设备播放音乐
        xiaoai
            .play_url(&device_id, "http://music-url")
            .await
            .unwrap();

        // 控制小爱的播放状态，比如让它停止
        xiaoai
            .set_play_state(&device_id, PlayState::Stop)
            .await
            .unwrap();

        // 让小爱执行文本，效果就跟口头询问一样
        xiaoai.nlp(&device_id, "查询今天的天气").await.unwrap();

        // 还可以进行低层次的请求，比如 Ubus Call
        let response = xiaoai
            .ubus_call(&device_id, "mibrain", "nlp_result_get", "{}")
            .await
            .unwrap();

        // 通过响应体了解请求的结果
        println!("{}", response.data);
    }
}
```

参见 [examples](/miai/examples/) 文件夹以获得更多示例。

## 许可证

本项目通过 [MIT license](/LICENSE) 授权。
