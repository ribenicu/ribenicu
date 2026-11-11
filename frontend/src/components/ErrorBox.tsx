/* 加载失败提示框 */
export default function ErrorBox({ message }: { message?: string }) {
  return <div className="load-error">📭 {message || '加载失败，请稍后再试'}</div>;
}
