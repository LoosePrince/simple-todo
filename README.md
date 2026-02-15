# 简易代办

自用桌面待办应用，基于 Tauri 2 + Vue 3。

## 技术栈

- **前端**: Vue 3、Vite、Element Plus、Pinia、Vue Router、vue-i18n
- **桌面**: Tauri 2（Rust）
- **功能**: 待办列表、富文本详情（图片/文件）、设置（主题/语言/开机启动等）

## 开发

```bash
# 安装依赖
npm install

# 开发模式（热更新）
npm run tauri dev

# 仅前端预览
npm run dev
```

## 构建

```bash
npm run build
# 再在 src-tauri 下用 cargo 打安装包，或通过 Tauri 命令打包
npm run tauri build
```

## 数据

数据与资源保存在本地配置目录（设置里可查看/修改路径），按条目分文件夹存储，图片与附件在 `assets` 子目录。

---

个人项目，仅供自用。
