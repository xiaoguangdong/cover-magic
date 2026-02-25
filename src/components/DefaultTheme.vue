<template>
  <div class="flex justify-center">
  <canvas ref="previewCanvas" class="border-2 border-gray-300 rounded-lg shadow-lg w-full max-w-4xl" :width="1920"
    :height="1080" style="width: 100%; height: auto"></canvas>
    </div>
</template>

<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { ref, onMounted, nextTick, watch } from "vue";

import type { BackgroundConfig, IconConfig, TitleConfig, WatermarkConfig, ExportConfig } from "@/lib/type";

import { BACKGROUND_TYPE, GRADIENT_DIRECTION } from "@/lib/constant";

// 定义组件的属性
const props = defineProps<{
  // 背景配置：控制封面背景的各项属性
  backgroundConfig: BackgroundConfig;
  // 图标配置：控制封面中心图标的各项属性
  iconConfig: IconConfig;
  // 标题配置：控制封面标题文本的各项属性
  titleConfig: TitleConfig;
  // 水印配置：控制封面水印的各项属性
  watermarkConfig: WatermarkConfig;
}>();

type Emits = {
  // 画布更新事件：当画布内容发生变化时触发
  // eslint-disable-next-line no-unused-vars
  (_e: "canvas-update"): void;
};

// 定义组件的事件
const emit = defineEmits<Emits>();

// 画布引用和缓存变量
const previewCanvas = ref<HTMLCanvasElement>();
let iconImageCache: HTMLImageElement | null = null;
let updateTimer: number | null = null;
let isUpdating = false;
let fontLoaded = false;

const isRunningInTauri = () => {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
};

/**
 * 获取带有回退选项的字体系列字符串
 * @param font 主要字体名称
 * @returns 包含回退字体的完整字体系列字符串
 */
const getFontFamilyWithFallback = (font: string): string => {
  switch (font) {
    case "Maple Mono CN":
      return '"Maple Mono CN", "Courier New", "Consolas", monospace';
    case "Fira Code":
      return '"Fira Code", "Courier New", "Consolas", monospace';
    case "Lato":
      return '"Lato", "Helvetica Neue", "Arial", sans-serif';
    case "Microsoft YaHei":
      return '"Microsoft YaHei", "微软雅黑", "SimHei", "Arial Unicode MS", sans-serif';
    case "PingFang SC":
      return '"PingFang SC", "苹方", "Helvetica Neue", "Arial", sans-serif';
    case "Source Han Sans CN":
      return '"Source Han Sans CN", "思源黑体", "Noto Sans CJK SC", "Microsoft YaHei", sans-serif';
    case "Arial":
      return 'Arial, "Helvetica Neue", Helvetica, sans-serif';
    case "Helvetica":
      return 'Helvetica, "Helvetica Neue", Arial, sans-serif';
    case "monospace":
      return '"Courier New", Consolas, "Liberation Mono", monospace';
    case "serif":
      return '"Times New Roman", Times, "Songti SC", serif';
    default:
      return `"${font}", Arial, sans-serif`;
  }
};

// 检测字体是否加载完成
const checkFontLoaded = async () => {
  if (fontLoaded) return true;

  try {
    // 使用 document.fonts.ready 检测字体加载
    await document.fonts.ready;

    // 额外检查 Maple Mono CN 字体是否可用
    const testCanvas = document.createElement("canvas");
    const testCtx = testCanvas.getContext("2d");
    if (testCtx) {
      testCtx.font = "16px Maple Mono CN";
      const mapleWidth = testCtx.measureText("测试").width;

      testCtx.font = "16px monospace";
      const fallbackWidth = testCtx.measureText("测试").width;

      // 如果宽度不同，说明 Maple Mono CN 已加载
      fontLoaded = Math.abs(mapleWidth - fallbackWidth) > 0.1;
    }

    return fontLoaded;
  } catch (error) {
    console.warn("字体检测失败:", error);
    fontLoaded = true; // 假设已加载，避免无限等待
    return true;
  }
};

// 等待字体加载的辅助函数
const waitForFont = async (maxWait = 3000) => {
  const startTime = Date.now();

  while (!fontLoaded && Date.now() - startTime < maxWait) {
    await checkFontLoaded();
    if (!fontLoaded) {
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
  }

  return fontLoaded;
};

// 绘制阴影
const drawShadow = (ctx: CanvasRenderingContext2D) => {
  if (props.iconConfig.shadowSize > 0) {
    ctx.shadowColor = props.iconConfig.shadowColor;
    ctx.shadowBlur = props.iconConfig.shadowSize;
    ctx.shadowOffsetX = 0;
    ctx.shadowOffsetY = 0;
  }
};

// 清除阴影
const clearShadow = (ctx: CanvasRenderingContext2D) => {
  ctx.shadowColor = "transparent";
  ctx.shadowBlur = 0;
  ctx.shadowOffsetX = 0;
  ctx.shadowOffsetY = 0;
};

// 绘制 SVG 图标（使用缓存）
const drawIcon = async (ctx: CanvasRenderingContext2D, x: number, y: number, size: number) => {
  if (!props.iconConfig.svg) return;

  // 如果缓存的图标存在且可用，直接使用
  if (iconImageCache) {
    // 设置阴影
    drawShadow(ctx);

    // 绘制图标，保持宽高比
    const aspectRatio = iconImageCache.width / iconImageCache.height;
    let drawWidth = size;
    let drawHeight = size;

    if (aspectRatio > 1) {
      drawHeight = size / aspectRatio;
    } else {
      drawWidth = size * aspectRatio;
    }

    ctx.drawImage(iconImageCache, x - drawWidth / 2, y - drawHeight / 2, drawWidth, drawHeight);

    // 清除阴影
    clearShadow(ctx);
    return;
  }

  // 如果没有缓存，创建新的图标
  return new Promise<void>((resolve) => {
    const svgBlob = new Blob([props.iconConfig.svg], { type: "image/svg+xml" });
    const url = URL.createObjectURL(svgBlob);
    const img = new Image();

    img.onload = () => {
      // 缓存图标
      iconImageCache = img;

      // 设置阴影
      drawShadow(ctx);

      // 绘制图标，保持宽高比
      const aspectRatio = img.width / img.height;
      let drawWidth = size;
      let drawHeight = size;

      if (aspectRatio > 1) {
        drawHeight = size / aspectRatio;
      } else {
        drawWidth = size * aspectRatio;
      }

      ctx.drawImage(img, x - drawWidth / 2, y - drawHeight / 2, drawWidth, drawHeight);

      // 清除阴影
      clearShadow(ctx);

      URL.revokeObjectURL(url);
      resolve();
    };

    img.onerror = () => {
      URL.revokeObjectURL(url);
      resolve();
    };

    img.src = url;
  });
};

// 存储动画状态
interface AnimationState {
  value: number;
  target: number;
  velocity: number;
}

// 创建动画状态对象
const createAnimationState = (initialValue: number = 0): AnimationState => ({
  value: initialValue,
  target: initialValue,
  velocity: 0,
});

// 存储所有需要动画的属性状态
const animationStates = {
  titleSize: createAnimationState(),
  titleX: createAnimationState(),
  titleY: createAnimationState(),
  watermarkSize: createAnimationState(),
  watermarkX: createAnimationState(),
  watermarkY: createAnimationState(),
  iconSize: createAnimationState(),
  iconX: createAnimationState(),
  iconY: createAnimationState(),
  watermarkOpacity: createAnimationState(),
  backgroundBlur: createAnimationState(),
};

// 弹簧动画参数
const SPRING_STIFFNESS = 200; // 弹簧刚度
const SPRING_DAMPING = 20; // 阻尼系数
const SPRING_MASS = 1; // 质量
const SPRING_PRECISION = 0.01; // 精度阈值

// 是否正在执行动画循环
let animationLoopRunning = false;

// 弹簧动画更新函数 - 使用物理模型实现更自然的动画
const updateSpringAnimation = (state: AnimationState, dt: number): boolean => {
  // 计算弹簧力
  const displacement = state.target - state.value;

  // 如果位移很小，直接到达目标位置
  if (Math.abs(displacement) < SPRING_PRECISION && Math.abs(state.velocity) < SPRING_PRECISION) {
    state.value = state.target;
    state.velocity = 0;
    return false; // 动画完成
  }

  // 计算弹簧力 (F = -kx)
  const springForce = SPRING_STIFFNESS * displacement;

  // 计算阻尼力 (F = -cv)
  const dampingForce = -SPRING_DAMPING * state.velocity;

  // 合力
  const force = springForce + dampingForce;

  // 加速度 (F = ma, a = F/m)
  const acceleration = force / SPRING_MASS;

  // 更新速度 (v = v0 + at)
  state.velocity += acceleration * dt;

  // 更新位置 (x = x0 + vt)
  state.value += state.velocity * dt;

  return true; // 动画继续
};

// 更新动画目标值
const updateAnimationTargets = () => {
  // 更新标题相关动画目标
  animationStates.titleSize.target = props.titleConfig.size;
  animationStates.titleX.target = props.titleConfig.position.x;
  animationStates.titleY.target = props.titleConfig.position.y;

  // 更新水印相关动画目标
  animationStates.watermarkSize.target = props.watermarkConfig.size;
  animationStates.watermarkX.target = props.watermarkConfig.position.x;
  animationStates.watermarkY.target = props.watermarkConfig.position.y;
  animationStates.watermarkOpacity.target = props.watermarkConfig.opacity;

  // 更新图标相关动画目标
  animationStates.iconSize.target = props.iconConfig.size;
  animationStates.iconX.target = props.iconConfig.position.x;
  animationStates.iconY.target = props.iconConfig.position.y;

  // 更新背景模糊动画目标
  animationStates.backgroundBlur.target = props.backgroundConfig.blur;

  // 如果是首次更新，直接设置当前值等于目标值
  if (animationStates.titleSize.value === 0) {
    Object.keys(animationStates).forEach((key) => {
      const state = animationStates[key as keyof typeof animationStates];
      state.value = state.target;
    });
  }

  // 启动动画循环（如果尚未运行）
  startAnimationLoop();
};

// 启动动画循环
const startAnimationLoop = () => {
  if (animationLoopRunning) return;

  animationLoopRunning = true;
  let lastTime = performance.now();

  const animationLoop = async () => {
    const currentTime = performance.now();
    const deltaTime = Math.min((currentTime - lastTime) / 1000, 0.1); // 秒为单位，限制最大时间步长
    lastTime = currentTime;

    // 更新所有动画状态
    let anyActive = false;
    Object.keys(animationStates).forEach((key) => {
      const state = animationStates[key as keyof typeof animationStates];
      if (updateSpringAnimation(state, deltaTime)) {
        anyActive = true;
      }
    });

    // 渲染当前帧
    await renderCurrentFrame();

    // 如果还有活动的动画，继续循环
    if (anyActive) {
      requestAnimationFrame(animationLoop);
    } else {
      animationLoopRunning = false;
    }
  };

  // 开始动画循环
  requestAnimationFrame(animationLoop);
};

// 防抖更新画布 - 只更新目标值，不直接渲染
const debouncedUpdateCanvas = () => {
  if (updateTimer) {
    clearTimeout(updateTimer);
  }

  updateTimer = setTimeout(() => {
    updateAnimationTargets();
  }, 5); // 使用更短的延迟，提高响应性
};

// 渲染当前帧
const renderCurrentFrame = async () => {
  if (isUpdating) return;
  isUpdating = true;

  await nextTick();

  const canvas = previewCanvas.value;
  if (!canvas) {
    isUpdating = false;
    return;
  }

  const ctx = canvas.getContext("2d");
  if (!ctx) {
    isUpdating = false;
    return;
  }

  // 等待字体加载完成
  await waitForFont();

  // 使用requestAnimationFrame确保平滑渲染
  requestAnimationFrame(async () => {
    // 获取当前动画值
    const currentTitleX = animationStates.titleX.value;
    const currentTitleY = animationStates.titleY.value;
    const currentWatermarkX = animationStates.watermarkX.value;
    const currentWatermarkY = animationStates.watermarkY.value;
    const currentIconSize = animationStates.iconSize.value;
    const currentIconX = animationStates.iconX.value;
    const currentIconY = animationStates.iconY.value;

    // 渲染当前帧
    await renderFrame(
      currentTitleX,
      currentTitleY,
      currentWatermarkX,
      currentWatermarkY,
      currentIconSize,
      currentIconX,
      currentIconY
    );

    isUpdating = false;
  });
};

// 渲染单个帧
const renderFrame = async (
  currentTitleX: number,
  currentTitleY: number,
  currentWatermarkX: number,
  currentWatermarkY: number,
  currentIconSize: number,
  currentIconX: number,
  currentIconY: number
) => {
  const canvas = previewCanvas.value;
  if (!canvas) return;

  const ctx = canvas.getContext("2d", { alpha: false });
  if (!ctx) return;

  // 清空画布 - 使用填充而不是clearRect以减少闪烁
  ctx.fillStyle = "#ffffff";
  ctx.fillRect(0, 0, canvas.width, canvas.height);

  // 绘制背景
  if (props.backgroundConfig.type === BACKGROUND_TYPE.COLOR) {
    // 绘制纯色背景，应用透明度
    const opacity = props.backgroundConfig.opacity / 100;
    const color = props.backgroundConfig.color;

    // 将十六进制颜色转换为 rgba 格式
    const r = parseInt(color.slice(1, 3), 16);
    const g = parseInt(color.slice(3, 5), 16);
    const b = parseInt(color.slice(5, 7), 16);

    ctx.fillStyle = `rgba(${r}, ${g}, ${b}, ${opacity})`;
    ctx.fillRect(0, 0, canvas.width, canvas.height);
  } else if (props.backgroundConfig.type === BACKGROUND_TYPE.GRADIENT) {
    // 绘制渐变背景
    const { startColor, endColor, direction } = props.backgroundConfig.gradient;
    const opacity = props.backgroundConfig.opacity / 100;

    // 创建渐变对象
    let gradient;

    switch (direction) {
      case GRADIENT_DIRECTION.TO_RIGHT:
        gradient = ctx.createLinearGradient(0, 0, canvas.width, 0);
        break;
      case GRADIENT_DIRECTION.TO_LEFT:
        gradient = ctx.createLinearGradient(canvas.width, 0, 0, 0);
        break;
      case GRADIENT_DIRECTION.TO_BOTTOM:
        gradient = ctx.createLinearGradient(0, 0, 0, canvas.height);
        break;
      case GRADIENT_DIRECTION.TO_TOP:
        gradient = ctx.createLinearGradient(0, canvas.height, 0, 0);
        break;
      case GRADIENT_DIRECTION.TO_BOTTOM_RIGHT:
        gradient = ctx.createLinearGradient(0, 0, canvas.width, canvas.height);
        break;
      case GRADIENT_DIRECTION.TO_BOTTOM_LEFT:
        gradient = ctx.createLinearGradient(canvas.width, 0, 0, canvas.height);
        break;
      case GRADIENT_DIRECTION.TO_TOP_RIGHT:
        gradient = ctx.createLinearGradient(0, canvas.height, canvas.width, 0);
        break;
      case GRADIENT_DIRECTION.TO_TOP_LEFT:
        gradient = ctx.createLinearGradient(canvas.width, canvas.height, 0, 0);
        break;
      default:
        gradient = ctx.createLinearGradient(0, 0, canvas.width, canvas.height);
    }

    // 将十六进制颜色转换为 rgba 格式
    const startR = parseInt(startColor.slice(1, 3), 16);
    const startG = parseInt(startColor.slice(3, 5), 16);
    const startB = parseInt(startColor.slice(5, 7), 16);

    const endR = parseInt(endColor.slice(1, 3), 16);
    const endG = parseInt(endColor.slice(3, 5), 16);
    const endB = parseInt(endColor.slice(5, 7), 16);

    // 添加渐变色标
    gradient.addColorStop(0, `rgba(${startR}, ${startG}, ${startB}, ${opacity})`);
    gradient.addColorStop(1, `rgba(${endR}, ${endG}, ${endB}, ${opacity})`);

    // 应用渐变
    ctx.fillStyle = gradient;
    ctx.fillRect(0, 0, canvas.width, canvas.height);
  } else if (props.backgroundConfig.imageObj) {
    // 绘制背景图片
    // 只有当模糊值大于0时才应用模糊滤镜
    if (props.backgroundConfig.blur > 0) {
      ctx.filter = `blur(${props.backgroundConfig.blur}px)`;
    } else {
      ctx.filter = "none";
    }
    
    // 保持图片宽高比并保持清晰度
    const img = props.backgroundConfig.imageObj;
    const imgRatio = img.width / img.height;
    const canvasRatio = canvas.width / canvas.height;
    
    // 创建离屏画布以保持图片质量
    const offscreenCanvas = document.createElement('canvas');
    const offscreenCtx = offscreenCanvas.getContext('2d', { alpha: false });
    
    if (!offscreenCtx) {
      // 如果离屏画布创建失败，回退到直接绘制
      let drawWidth = canvas.width;
      let drawHeight = canvas.height;
      let offsetX = 0;
      let offsetY = 0;
      
      // 根据宽高比决定如何填充画布
      if (imgRatio > canvasRatio) {
        drawHeight = canvas.height;
        drawWidth = drawHeight * imgRatio;
        offsetX = (canvas.width - drawWidth) / 2;
      } else {
        drawWidth = canvas.width;
        drawHeight = drawWidth / imgRatio;
        offsetY = (canvas.height - drawHeight) / 2;
      }
      
      ctx.drawImage(img, offsetX, offsetY, drawWidth, drawHeight);
    } else {
      // 使用离屏画布处理图片，保持原始分辨率
      offscreenCanvas.width = img.width;
      offscreenCanvas.height = img.height;
      
      // 在离屏画布上绘制原始图片
      offscreenCtx.drawImage(img, 0, 0, img.width, img.height);
      
      // 计算绘制参数
      let drawWidth = canvas.width;
      let drawHeight = canvas.height;
      let offsetX = 0;
      let offsetY = 0;
      
      // 根据宽高比决定如何填充画布
      if (imgRatio > canvasRatio) {
        drawHeight = canvas.height;
        drawWidth = drawHeight * imgRatio;
        offsetX = (canvas.width - drawWidth) / 2;
      } else {
        drawWidth = canvas.width;
        drawHeight = drawWidth / imgRatio;
        offsetY = (canvas.height - drawHeight) / 2;
      }
      
      // 使用高质量的图像绘制方法
      ctx.imageSmoothingEnabled = true;
      ctx.imageSmoothingQuality = 'high';
      
      // 将离屏画布内容绘制到主画布
      ctx.drawImage(offscreenCanvas, offsetX, offsetY, drawWidth, drawHeight);
    }
    ctx.filter = "none";
  }

  // 绘制图标
  if (props.iconConfig.svg) {
    // 测量图标大小（假设为正方形）
    const iconSize = currentIconSize;
    const halfIconSize = iconSize / 2;

    let iconPosX, iconPosY;

    // 水平位置计算
    if (currentIconX <= 0) {
      // 左对齐
      iconPosX = halfIconSize;
    } else if (currentIconX >= 100) {
      // 右对齐
      iconPosX = canvas.width - halfIconSize;
    } else {
      // 在0%到100%之间线性插值
      const minX = halfIconSize;
      const maxX = canvas.width - halfIconSize;
      iconPosX = minX + (maxX - minX) * (currentIconX / 100);
    }

    // 垂直位置计算
    if (currentIconY <= 0) {
      // 顶部位置
      iconPosY = halfIconSize;
    } else if (currentIconY >= 100) {
      // 底部位置
      iconPosY = canvas.height - halfIconSize;
    } else {
      // 在0%到100%之间线性插值
      const minY = halfIconSize;
      const maxY = canvas.height - halfIconSize;
      iconPosY = minY + (maxY - minY) * (currentIconY / 100);
    }

    await drawIcon(ctx, iconPosX, iconPosY, currentIconSize);
  }

  // 绘制标题
  if (props.titleConfig.text) {
    ctx.save();
    ctx.fillStyle = props.titleConfig.color;

    // 设置字体以便测量文本宽度
    const titleFontFamily = getFontFamilyWithFallback(props.titleConfig.font);
    const titleFontWeight = props.titleConfig.effects.bold ? "bold" : "normal";
    ctx.font = `${titleFontWeight} ${props.titleConfig.size}px ${titleFontFamily}`;
    
    // 设置立体字效果（文字阴影）
    if (props.titleConfig.effects.textShadow && props.titleConfig.effects.textShadow > 0) {
      const shadowSize = props.titleConfig.effects.textShadow;
      const shadowColor = props.titleConfig.color;
      ctx.shadowColor = shadowColor;
      ctx.shadowBlur = shadowSize * 2;
      ctx.shadowOffsetX = shadowSize;
      ctx.shadowOffsetY = shadowSize;
    }

    // 测量文本宽度和高度
    const titleTextWidth = ctx.measureText(props.titleConfig.text).width;
    const titleTextHeight = props.titleConfig.size; // 近似文本高度

    let titlePosX, titlePosY;

    // 水平位置计算
    if (currentTitleX <= 0) {
      // 左对齐
      ctx.textAlign = "left";
      titlePosX = 0;
    } else if (currentTitleX >= 100) {
      // 右对齐
      ctx.textAlign = "right";
      titlePosX = canvas.width;
    } else {
      // 在0%到100%之间线性插值，考虑文本宽度
      ctx.textAlign = "center";
      // 文本居中时，可滑动范围是从 titleTextWidth/2 到 canvas.width - titleTextWidth/2
      const minX = titleTextWidth / 2;
      const maxX = canvas.width - titleTextWidth / 2;
      titlePosX = minX + (maxX - minX) * (currentTitleX / 100);
    }

    // 垂直位置计算 - 始终使用中线对齐，避免切换基线导致的抖动
    ctx.textBaseline = "middle";

    // 计算文本高度的一半，用于边缘位置的调整
    const halfTitleHeight = titleTextHeight / 2;

    if (currentTitleY <= 0) {
      // 顶部位置
      titlePosY = halfTitleHeight; // 距离顶部半个文本高度
    } else if (currentTitleY >= 100) {
      // 底部位置
      titlePosY = canvas.height - halfTitleHeight; // 距离底部半个文本高度
    } else {
      // 在0%到100%之间线性插值
      const minY = halfTitleHeight;
      const maxY = canvas.height - halfTitleHeight;
      titlePosY = minY + (maxY - minY) * (currentTitleY / 100);
    }

    // 更新字体设置 - 只设置加粗，斜体用变换实现
    ctx.font = `${titleFontWeight} ${props.titleConfig.size}px ${titleFontFamily}`;

    // 如果需要斜体，使用变换
    if (props.titleConfig.effects.italic) {
      ctx.translate(titlePosX, titlePosY);
      ctx.transform(1, 0, -0.2, 1, 0, 0);

      // 特殊处理 Maple Mono CN 的加粗效果
      if (props.titleConfig.effects.bold && props.titleConfig.font === "Maple Mono CN") {
        // 使用描边模拟加粗效果
        ctx.strokeStyle = props.titleConfig.color;
        ctx.lineWidth = props.titleConfig.size * 2 * 0.01; // 根据字体大小调整描边宽度（导出时使用2倍缩放）
        ctx.strokeText(props.titleConfig.text, 0, 0);
      }
      ctx.fillText(props.titleConfig.text, 0, 0);
    } else {
      // 特殊处理 Maple Mono CN 的加粗效果
      if (props.titleConfig.effects.bold && props.titleConfig.font === "Maple Mono CN") {
        // 使用描边模拟加粗效果
        ctx.strokeStyle = props.titleConfig.color;
        ctx.lineWidth = props.titleConfig.size * 2 * 0.01; // 根据字体大小调整描边宽度
        ctx.strokeText(props.titleConfig.text, titlePosX, titlePosY);
      }
      ctx.fillText(props.titleConfig.text, titlePosX, titlePosY);
    }
    ctx.restore();
  }

  // 绘制水印
  if (props.watermarkConfig.text) {
    const watermarkOpacity = props.watermarkConfig.opacity / 100;

    ctx.save();
    ctx.globalAlpha = watermarkOpacity;
    ctx.fillStyle = props.watermarkConfig.color;

    // 处理大写转换
    const displayWatermarkText = props.watermarkConfig.effects.uppercase
      ? props.watermarkConfig.text.toUpperCase()
      : props.watermarkConfig.text;

    // 设置字体以便测量文本宽度
    const watermarkFontFamily = getFontFamilyWithFallback(props.watermarkConfig.font);
    const watermarkFontWeight = props.watermarkConfig.effects.bold ? "bold" : "normal";
    ctx.font = `${watermarkFontWeight} ${props.watermarkConfig.size}px ${watermarkFontFamily}`;

    // 测量文本宽度和高度
    const watermarkTextWidth = ctx.measureText(displayWatermarkText).width;
    const watermarkTextHeight = props.watermarkConfig.size; // 近似文本高度

    let watermarkPosX, watermarkPosY;

    // 水平位置计算
    if (currentWatermarkX <= 0) {
      // 左对齐
      ctx.textAlign = "left";
      watermarkPosX = 0;
    } else if (currentWatermarkX >= 100) {
      // 右对齐
      ctx.textAlign = "right";
      watermarkPosX = canvas.width;
    } else {
      // 在0%到100%之间线性插值，考虑文本宽度
      ctx.textAlign = "center";
      // 文本居中时，可滑动范围是从 watermarkTextWidth/2 到 canvas.width - watermarkTextWidth/2
      const minX = watermarkTextWidth / 2;
      const maxX = canvas.width - watermarkTextWidth / 2;
      watermarkPosX = minX + (maxX - minX) * (currentWatermarkX / 100);
    }

    // 垂直位置计算 - 始终使用中线对齐，避免切换基线导致的抖动
    ctx.textBaseline = "middle";

    // 计算文本高度的一半，用于边缘位置的调整
    const halfWatermarkHeight = watermarkTextHeight / 2;

    if (currentWatermarkY <= 0) {
      // 顶部位置
      watermarkPosY = halfWatermarkHeight; // 距离顶部半个文本高度
    } else if (currentWatermarkY >= 100) {
      // 底部位置
      watermarkPosY = canvas.height - halfWatermarkHeight; // 距离底部半个文本高度
    } else {
      // 在0%到100%之间线性插值
      const minY = halfWatermarkHeight;
      const maxY = canvas.height - halfWatermarkHeight;
      watermarkPosY = minY + (maxY - minY) * (currentWatermarkY / 100);
    }

    // 更新字体设置
    ctx.font = `${watermarkFontWeight} ${props.watermarkConfig.size}px ${watermarkFontFamily}`;

    // 如果需要斜体，使用变换
    if (props.watermarkConfig.effects.italic) {
      ctx.translate(watermarkPosX, watermarkPosY);
      ctx.transform(1, 0, -0.2, 1, 0, 0);

      // 特殊处理 Maple Mono CN 的加粗效果
      if (props.watermarkConfig.effects.bold && props.watermarkConfig.font === "Maple Mono CN") {
        // 使用描边模拟加粗效果
        ctx.strokeStyle = props.watermarkConfig.color;
        ctx.lineWidth = props.watermarkConfig.size * 0.01; // 根据字体大小调整描边宽度
        ctx.strokeText(displayWatermarkText, 0, 0);
      }
      ctx.fillText(displayWatermarkText, 0, 0);
    } else {
      // 特殊处理 Maple Mono CN 的加粗效果
      if (props.watermarkConfig.effects.bold && props.watermarkConfig.font === "Maple Mono CN") {
        // 使用描边模拟加粗效果
        ctx.strokeStyle = props.watermarkConfig.color;
        ctx.lineWidth = props.watermarkConfig.size * 0.01; // 根据字体大小调整描边宽度
        ctx.strokeText(displayWatermarkText, watermarkPosX, watermarkPosY);
      }
      ctx.fillText(displayWatermarkText, watermarkPosX, watermarkPosY);
    }
    ctx.restore();
  }
};

// 更新画布 - 公共入口点
const updateCanvas = () => {
  debouncedUpdateCanvas();
  emit("canvas-update");
};

// 监听配置变化，更新画布
watch(
  () => [props.backgroundConfig, props.iconConfig, props.titleConfig, props.watermarkConfig],
  () => {
    updateCanvas();
  },
  { deep: true }
);

// 当图标SVG内容变化时，清除缓存
watch(
  () => props.iconConfig.svg,
  () => {
    iconImageCache = null;
  }
);

// 组件挂载时初始化
onMounted(async () => {
  // 等待字体加载
  await waitForFont();

  // 初始化画布
  updateCanvas();
});

// 导出图片函数
const exportImage = async (exportConfig: ExportConfig) => {
  const canvas = previewCanvas.value;
  if (!canvas) return;

  // 创建一个新的离屏画布，用于导出
  const exportCanvas = document.createElement("canvas");
  exportCanvas.width = exportConfig.width;
  exportCanvas.height = exportConfig.height;

  const exportCtx = exportCanvas.getContext("2d", { alpha: false });
  if (!exportCtx) return;

  // 清空导出画布
  exportCtx.fillStyle = "#ffffff";
  exportCtx.fillRect(0, 0, exportCanvas.width, exportCanvas.height);

  // 绘制背景
  if (props.backgroundConfig.type === BACKGROUND_TYPE.COLOR) {
    // 绘制纯色背景，应用透明度
    const opacity = props.backgroundConfig.opacity / 100;
    const color = props.backgroundConfig.color;

    // 将十六进制颜色转换为 rgba 格式
    const r = parseInt(color.slice(1, 3), 16);
    const g = parseInt(color.slice(3, 5), 16);
    const b = parseInt(color.slice(5, 7), 16);

    exportCtx.fillStyle = `rgba(${r}, ${g}, ${b}, ${opacity})`;
    exportCtx.fillRect(0, 0, exportCanvas.width, exportCanvas.height);
  } else if (props.backgroundConfig.type === BACKGROUND_TYPE.GRADIENT) {
    // 绘制渐变背景
    const { startColor, endColor, direction } = props.backgroundConfig.gradient;
    const opacity = props.backgroundConfig.opacity / 100;

    // 创建渐变对象
    let gradient;

    switch (direction) {
      case GRADIENT_DIRECTION.TO_RIGHT:
        gradient = exportCtx.createLinearGradient(0, 0, exportCanvas.width, 0);
        break;
      case GRADIENT_DIRECTION.TO_LEFT:
        gradient = exportCtx.createLinearGradient(exportCanvas.width, 0, 0, 0);
        break;
      case GRADIENT_DIRECTION.TO_BOTTOM:
        gradient = exportCtx.createLinearGradient(0, 0, 0, exportCanvas.height);
        break;
      case GRADIENT_DIRECTION.TO_TOP:
        gradient = exportCtx.createLinearGradient(0, exportCanvas.height, 0, 0);
        break;
      case GRADIENT_DIRECTION.TO_BOTTOM_RIGHT:
        gradient = exportCtx.createLinearGradient(0, 0, exportCanvas.width, exportCanvas.height);
        break;
      case GRADIENT_DIRECTION.TO_BOTTOM_LEFT:
        gradient = exportCtx.createLinearGradient(exportCanvas.width, 0, 0, exportCanvas.height);
        break;
      case GRADIENT_DIRECTION.TO_TOP_RIGHT:
        gradient = exportCtx.createLinearGradient(0, exportCanvas.height, exportCanvas.width, 0);
        break;
      case GRADIENT_DIRECTION.TO_TOP_LEFT:
        gradient = exportCtx.createLinearGradient(exportCanvas.width, exportCanvas.height, 0, 0);
        break;
      default:
        gradient = exportCtx.createLinearGradient(0, 0, exportCanvas.width, exportCanvas.height);
    }

    // 将十六进制颜色转换为 rgba 格式
    const startR = parseInt(startColor.slice(1, 3), 16);
    const startG = parseInt(startColor.slice(3, 5), 16);
    const startB = parseInt(startColor.slice(5, 7), 16);

    const endR = parseInt(endColor.slice(1, 3), 16);
    const endG = parseInt(endColor.slice(3, 5), 16);
    const endB = parseInt(endColor.slice(5, 7), 16);

    // 添加渐变色标
    gradient.addColorStop(0, `rgba(${startR}, ${startG}, ${startB}, ${opacity})`);
    gradient.addColorStop(1, `rgba(${endR}, ${endG}, ${endB}, ${opacity})`);

    // 应用渐变
    exportCtx.fillStyle = gradient;
    exportCtx.fillRect(0, 0, exportCanvas.width, exportCanvas.height);
  } else if (props.backgroundConfig.imageObj) {
    // 绘制背景图片
    // 只有当模糊值大于0时才应用模糊滤镜
    if (props.backgroundConfig.blur > 0) {
      exportCtx.filter = `blur(${props.backgroundConfig.blur}px)`;
    } else {
      exportCtx.filter = "none";
    }
    
    // 保持图片宽高比并保持清晰度
    const img = props.backgroundConfig.imageObj;
    const imgRatio = img.width / img.height;
    const canvasRatio = exportCanvas.width / exportCanvas.height;
    
    // 创建离屏画布以保持图片质量
    const offscreenCanvas = document.createElement('canvas');
    const offscreenCtx = offscreenCanvas.getContext('2d', { alpha: false });
    
    if (!offscreenCtx) {
      // 如果离屏画布创建失败，回退到直接绘制
      let drawWidth = exportCanvas.width;
      let drawHeight = exportCanvas.height;
      let offsetX = 0;
      let offsetY = 0;
      
      // 根据宽高比决定如何填充画布
      if (imgRatio > canvasRatio) {
        drawHeight = exportCanvas.height;
        drawWidth = drawHeight * imgRatio;
        offsetX = (exportCanvas.width - drawWidth) / 2;
      } else {
        drawWidth = exportCanvas.width;
        drawHeight = drawWidth / imgRatio;
        offsetY = (exportCanvas.height - drawHeight) / 2;
      }
      
      exportCtx.drawImage(img, offsetX, offsetY, drawWidth, drawHeight);
    } else {
      // 使用离屏画布处理图片，保持原始分辨率
      offscreenCanvas.width = img.width;
      offscreenCanvas.height = img.height;
      
      // 在离屏画布上绘制原始图片
      offscreenCtx.drawImage(img, 0, 0, img.width, img.height);
      
      // 计算绘制参数
      let drawWidth = exportCanvas.width;
      let drawHeight = exportCanvas.height;
      let offsetX = 0;
      let offsetY = 0;
      
      // 根据宽高比决定如何填充画布
      if (imgRatio > canvasRatio) {
        drawHeight = exportCanvas.height;
        drawWidth = drawHeight * imgRatio;
        offsetX = (exportCanvas.width - drawWidth) / 2;
      } else {
        drawWidth = exportCanvas.width;
        drawHeight = drawWidth / imgRatio;
        offsetY = (exportCanvas.height - drawHeight) / 2;
      }
      
      // 使用高质量的图像绘制方法
      exportCtx.imageSmoothingEnabled = true;
      exportCtx.imageSmoothingQuality = 'high';
      
      // 将离屏画布内容绘制到导出画布
      exportCtx.drawImage(offscreenCanvas, offsetX, offsetY, drawWidth, drawHeight);
    }
    exportCtx.filter = "none";
  }

  // 绘制图标
  if (props.iconConfig.svg) {
    const iconSize = props.iconConfig.size * (exportCanvas.width / canvas.width); // 按比例缩放
    const halfIconSize = iconSize / 2;

    let adjustedIconPosX, adjustedIconPosY;

    // 水平位置计算
    if (props.iconConfig.position.x <= 0) {
      // 左对齐
      adjustedIconPosX = halfIconSize;
    } else if (props.iconConfig.position.x >= 100) {
      // 右对齐
      adjustedIconPosX = exportCanvas.width - halfIconSize;
    } else {
      // 在0%到100%之间线性插值
      const minX = halfIconSize;
      const maxX = exportCanvas.width - halfIconSize;
      adjustedIconPosX = minX + (maxX - minX) * (props.iconConfig.position.x / 100);
    }

    // 垂直位置计算
    if (props.iconConfig.position.y <= 0) {
      // 顶部位置
      adjustedIconPosY = halfIconSize;
    } else if (props.iconConfig.position.y >= 100) {
      // 底部位置
      adjustedIconPosY = exportCanvas.height - halfIconSize;
    } else {
      // 在0%到100%之间线性插值
      const minY = halfIconSize;
      const maxY = exportCanvas.height - halfIconSize;
      adjustedIconPosY = minY + (maxY - minY) * (props.iconConfig.position.y / 100);
    }

    // 使用 SVG 数据 URL 创建图像
    const svgBlob = new Blob([props.iconConfig.svg], { type: "image/svg+xml" });
    const url = URL.createObjectURL(svgBlob);
    const img = new Image();

    await new Promise<void>((resolve) => {
      img.onload = () => {
        // 设置阴影
        if (props.iconConfig.shadowSize > 0) {
          exportCtx.shadowColor = props.iconConfig.shadowColor;
          exportCtx.shadowBlur = props.iconConfig.shadowSize * (exportCanvas.width / canvas.width); // 按比例缩放
          exportCtx.shadowOffsetX = 0;
          exportCtx.shadowOffsetY = 0;
        }

        // 绘制图标，保持宽高比
        const aspectRatio = img.width / img.height;
        let drawWidth = iconSize;
        let drawHeight = iconSize;

        if (aspectRatio > 1) {
          drawHeight = iconSize / aspectRatio;
        } else {
          drawWidth = iconSize * aspectRatio;
        }

        exportCtx.drawImage(
          img,
          adjustedIconPosX - drawWidth / 2,
          adjustedIconPosY - drawHeight / 2,
          drawWidth,
          drawHeight
        );

        // 清除阴影
        exportCtx.shadowColor = "transparent";
        exportCtx.shadowBlur = 0;
        exportCtx.shadowOffsetX = 0;
        exportCtx.shadowOffsetY = 0;

        URL.revokeObjectURL(url);
        resolve();
      };

      img.onerror = () => {
        URL.revokeObjectURL(url);
        resolve();
      };

      img.src = url;
    });
  }

  // 绘制标题
  if (props.titleConfig.text) {
    const titleSize = props.titleConfig.size * (exportCanvas.width / canvas.width); // 按比例缩放
    exportCtx.fillStyle = props.titleConfig.color;

    // 设置字体以便测量文本宽度
    const exportFontFamily = getFontFamilyWithFallback(props.titleConfig.font);
    const exportTitleFontWeight = props.titleConfig.effects.bold ? "bold" : "normal";
    exportCtx.font = `${exportTitleFontWeight} ${titleSize}px ${exportFontFamily}`;

    // 测量文本宽度和高度
    const textWidth = exportCtx.measureText(props.titleConfig.text).width;
    const textHeight = titleSize; // 近似文本高度

    let adjustedTitlePosX, adjustedTitlePosY;

    // 水平位置计算
    if (props.titleConfig.position.x <= 0) {
      // 左对齐
      exportCtx.textAlign = "left";
      adjustedTitlePosX = 0;
    } else if (props.titleConfig.position.x >= 100) {
      // 右对齐
      exportCtx.textAlign = "right";
      adjustedTitlePosX = exportCanvas.width;
    } else {
      // 在0%到100%之间线性插值，考虑文本宽度
      exportCtx.textAlign = "center";
      // 文本居中时，可滑动范围是从 textWidth/2 到 canvas.width - textWidth/2
      const minX = textWidth / 2;
      const maxX = exportCanvas.width - textWidth / 2;
      adjustedTitlePosX = minX + (maxX - minX) * (props.titleConfig.position.x / 100);
    }

    // 垂直位置计算 - 始终使用中线对齐，避免切换基线导致的抖动
    exportCtx.textBaseline = "middle";

    // 计算文本高度的一半，用于边缘位置的调整
    const halfTextHeight = textHeight / 2;

    if (props.titleConfig.position.y <= 0) {
      // 顶部位置
      adjustedTitlePosY = halfTextHeight; // 距离顶部半个文本高度
    } else if (props.titleConfig.position.y >= 100) {
      // 底部位置
      adjustedTitlePosY = exportCanvas.height - halfTextHeight; // 距离底部半个文本高度
    } else {
      // 在0%到100%之间线性插值
      const minY = halfTextHeight;
      const maxY = exportCanvas.height - halfTextHeight;
      adjustedTitlePosY = minY + (maxY - minY) * (props.titleConfig.position.y / 100);
    }

    // 更新字体设置
    exportCtx.font = `${exportTitleFontWeight} ${titleSize}px ${exportFontFamily}`;

    exportCtx.save();
    
    // 设置立体字效果（文字阴影）
    if (props.titleConfig.effects.textShadow && props.titleConfig.effects.textShadow > 0) {
      const shadowSize = props.titleConfig.effects.textShadow * (exportCanvas.width / canvas.width); // 按比例缩放
      const shadowColor = props.titleConfig.color;
      exportCtx.shadowColor = shadowColor;
      exportCtx.shadowBlur = shadowSize * 2;
      exportCtx.shadowOffsetX = shadowSize;
      exportCtx.shadowOffsetY = shadowSize;
    }
    // 如果需要斜体，使用变换
    if (props.titleConfig.effects.italic) {
      exportCtx.translate(adjustedTitlePosX, adjustedTitlePosY);
      exportCtx.transform(1, 0, -0.2, 1, 0, 0);

      // 特殊处理 Maple Mono CN 的加粗效果
      if (props.titleConfig.effects.bold && props.titleConfig.font === "Maple Mono CN") {
        // 使用描边模拟加粗效果
        exportCtx.strokeStyle = props.titleConfig.color;
        exportCtx.lineWidth = titleSize * 0.01; // 根据字体大小调整描边宽度
        exportCtx.strokeText(props.titleConfig.text, 0, 0);
      }
      exportCtx.fillText(props.titleConfig.text, 0, 0);
    } else {
      // 特殊处理 Maple Mono CN 的加粗效果
      if (props.titleConfig.effects.bold && props.titleConfig.font === "Maple Mono CN") {
        // 使用描边模拟加粗效果
        exportCtx.strokeStyle = props.titleConfig.color;
        exportCtx.lineWidth = titleSize * 0.01; // 根据字体大小调整描边宽度
        exportCtx.strokeText(props.titleConfig.text, adjustedTitlePosX, adjustedTitlePosY);
      }
      exportCtx.fillText(props.titleConfig.text, adjustedTitlePosX, adjustedTitlePosY);
    }
    exportCtx.restore();
  }

  // 绘制水印
  if (props.watermarkConfig.text) {
    const watermarkSize = props.watermarkConfig.size * (exportCanvas.width / canvas.width); // 按比例缩放
    const watermarkOpacity = props.watermarkConfig.opacity / 100;

    exportCtx.save();
    exportCtx.globalAlpha = watermarkOpacity;
    exportCtx.fillStyle = props.watermarkConfig.color;

    // 处理大写转换
    const exportDisplayWatermarkText = props.watermarkConfig.effects.uppercase
      ? props.watermarkConfig.text.toUpperCase()
      : props.watermarkConfig.text;

    // 设置字体以便测量文本宽度
    const exportWatermarkFontFamily = getFontFamilyWithFallback(props.watermarkConfig.font);
    const exportWatermarkFontWeight = props.watermarkConfig.effects.bold ? "bold" : "normal";
    exportCtx.font = `${exportWatermarkFontWeight} ${watermarkSize}px ${exportWatermarkFontFamily}`;

    // 测量文本宽度和高度
    const watermarkTextWidth = exportCtx.measureText(exportDisplayWatermarkText).width;
    const watermarkTextHeight = watermarkSize; // 近似文本高度

    let adjustedWatermarkPosX, adjustedWatermarkPosY;

    // 水平位置计算
    if (props.watermarkConfig.position.x <= 0) {
      // 左对齐
      exportCtx.textAlign = "left";
      adjustedWatermarkPosX = 0;
    } else if (props.watermarkConfig.position.x >= 100) {
      // 右对齐
      exportCtx.textAlign = "right";
      adjustedWatermarkPosX = exportCanvas.width;
    } else {
      // 在0%到100%之间线性插值，考虑文本宽度
      exportCtx.textAlign = "center";
      // 文本居中时，可滑动范围是从 watermarkTextWidth/2 到 canvas.width - watermarkTextWidth/2
      const minX = watermarkTextWidth / 2;
      const maxX = exportCanvas.width - watermarkTextWidth / 2;
      adjustedWatermarkPosX = minX + (maxX - minX) * (props.watermarkConfig.position.x / 100);
    }

    // 垂直位置计算 - 始终使用中线对齐，避免切换基线导致的抖动
    exportCtx.textBaseline = "middle";

    // 计算文本高度的一半，用于边缘位置的调整
    const halfWatermarkHeight = watermarkTextHeight / 2;

    if (props.watermarkConfig.position.y <= 0) {
      // 顶部位置
      adjustedWatermarkPosY = halfWatermarkHeight; // 距离顶部半个文本高度
    } else if (props.watermarkConfig.position.y >= 100) {
      // 底部位置
      adjustedWatermarkPosY = exportCanvas.height - halfWatermarkHeight; // 距离底部半个文本高度
    } else {
      // 在0%到100%之间线性插值
      const minY = halfWatermarkHeight;
      const maxY = exportCanvas.height - halfWatermarkHeight;
      adjustedWatermarkPosY = minY + (maxY - minY) * (props.watermarkConfig.position.y / 100);
    }

    // 更新字体设置
    exportCtx.font = `${exportWatermarkFontWeight} ${watermarkSize}px ${exportWatermarkFontFamily}`;

    // 如果需要斜体，使用变换
    if (props.watermarkConfig.effects.italic) {
      exportCtx.translate(adjustedWatermarkPosX, adjustedWatermarkPosY);
      exportCtx.transform(1, 0, -0.2, 1, 0, 0);

      // 特殊处理 Maple Mono CN 的加粗效果
      if (props.watermarkConfig.effects.bold && props.watermarkConfig.font === "Maple Mono CN") {
        // 使用描边模拟加粗效果
        exportCtx.strokeStyle = props.watermarkConfig.color;
        exportCtx.lineWidth = watermarkSize * 0.01; // 根据字体大小调整描边宽度
        exportCtx.strokeText(exportDisplayWatermarkText, 0, 0);
      }
      exportCtx.fillText(exportDisplayWatermarkText, 0, 0);
    } else {
      // 特殊处理 Maple Mono CN 的加粗效果
      if (props.watermarkConfig.effects.bold && props.watermarkConfig.font === "Maple Mono CN") {
        // 使用描边模拟加粗效果
        exportCtx.strokeStyle = props.watermarkConfig.color;
        exportCtx.lineWidth = watermarkSize * 0.01; // 根据字体大小调整描边宽度
        exportCtx.strokeText(exportDisplayWatermarkText, adjustedWatermarkPosX, adjustedWatermarkPosY);
      }
      exportCtx.fillText(exportDisplayWatermarkText, adjustedWatermarkPosX, adjustedWatermarkPosY);
    }
    exportCtx.restore();
  }

  // 获取最终文件名
  const getFinalFileName = () => {
    return exportConfig.useRandomFileName && exportConfig.currentRandomFileName
      ? exportConfig.currentRandomFileName
      : exportConfig.fileName;
  };

  const outputFileName = `${getFinalFileName()}.${exportConfig.format}`;

  // 导出图片
  // 使用质量参数控制压缩程度，1.0表示原始质量
  const dataURL = exportCanvas.toDataURL(`image/${exportConfig.format}`, exportConfig.quality);

  if (isRunningInTauri()) {
    try {
      const savePath = await save({
        title: "保存封面图片",
        defaultPath: outputFileName,
        filters: [
          {
            name: exportConfig.format.toUpperCase(),
            extensions: [exportConfig.format],
          },
        ],
      });

      if (!savePath) {
        return;
      }

      await invoke("save_export_image", {
        path: savePath,
        dataUrl: dataURL,
      });
      return;
    } catch (error) {
      console.error("桌面端保存失败，回退到浏览器下载逻辑:", error);
    }
  }

  // 创建下载链接
  const link = document.createElement("a");
  link.href = dataURL;
  link.download = outputFileName;
  document.body.appendChild(link);
  link.click();
  document.body.removeChild(link);
};

// 暴露方法给父组件
defineExpose({
  previewCanvas,
  updateCanvas,
  exportImage,
});
</script>
