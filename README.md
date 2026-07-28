# WASM-4 伪光线渲染

## 构建

```powershell
rustup target add wasm32-unknown-unknown
.\build.ps1
```

## 运行

WASM-4 原生运行器：

```powershell
.\w4.exe run-native dist\cart.wasm
```

浏览器：

```powershell
python -m http.server 8000
```

然后访问 <http://localhost:8000>。

方向键上/下控制前进后退，左/右控制转头；`Z` 抬头，`X` 低头。画面使用原生 160×160 分辨率和 WASM-4 经典默认四色调色板；视角变化时四帧交错更新，停止后两帧补齐完整画面。

场景主体包含盒体、球体、带顶盖圆柱和八面体；前景球体使用 100% 反射率和零粗糙度。碰撞按物体实际形状判断，斜向接触时会沿表面滑动。

静止时，高反射材质最多计算两次反射；第二次命中使用简化着色且不会继续反射。移动时自动降为一次反射。移动端网页提供虚拟摇杆以及抬头、低头触摸按钮。
