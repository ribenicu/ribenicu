/* 标签：颜色白名单（保留现有 tag 色系），用户输入经 JSX 转义 */
import type { Tag as TagData } from '../lib/api';

const COLORS = ['t-pink', 't-mint', 't-blue', 't-yellow'];

export default function Tag({ tag }: { tag: TagData }) {
  const color = COLORS.includes(tag.color) ? tag.color : 't-pink';
  return <span className={`tag ${color}`}>{tag.name}</span>;
}
