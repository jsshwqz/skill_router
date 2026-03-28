# space_navigation

本地运行时兜底技能。

当前机器缺少 Windows MSVC 链接环境，暂时无法把源码里的 `space_navigation` builtin 重新编译进正式 `aion-cli.exe`。
因此这里提供一个工作区本地 skill，把 `space_navigation` 先路由到 `builtin:placeholder`，保证当前交付物的 CLI 黑盒链路可用。

后续在完成正式重编译后，可将这里的入口改回 `builtin:space_navigation` 或升级为真实实现。
