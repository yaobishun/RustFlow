// 全局 ECharts 视觉基线：品牌色板、字体、tooltip 与图例样式。
// 各视图图表的 option 通过展开 baseChart 继承统一风格，再覆盖局部细节。

export const palette = [
  "#2161ee",
  "#10a7aa",
  "#ec9e24",
  "#8457dc",
  "#dc4f4f",
  "#1ba17c",
];

export const axisLabel = { color: "#98a2b3", fontSize: 11 };

export const baseChart = {
  color: palette,
  textStyle: {
    fontFamily: 'Inter, "Noto Sans SC", "Microsoft YaHei", sans-serif',
  },
  tooltip: {
    backgroundColor: "#101828",
    borderWidth: 0,
    padding: [8, 12],
    textStyle: { color: "#ffffff", fontSize: 12 },
    extraCssText: "border-radius:8px;box-shadow:0 8px 24px rgba(16,24,40,.18);",
  },
  legend: {
    icon: "roundRect",
    itemWidth: 10,
    itemHeight: 10,
    itemGap: 18,
    textStyle: { color: "#667085", fontSize: 12 },
  },
};

// 环形图中心叠加的汇总文本（left/top 按 288px 图高、44% 圆心设计）
export const donutTotal = (text: string, subtext: string) => ({
  text,
  subtext,
  left: "center",
  top: "34%",
  textAlign: "center",
  textStyle: { fontSize: 22, fontWeight: 700, color: "#172033" },
  subtextStyle: { fontSize: 11, color: "#98a2b3", lineHeight: 18 },
});
