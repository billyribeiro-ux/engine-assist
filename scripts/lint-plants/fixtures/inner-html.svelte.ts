export function sink(el: HTMLElement, s: string): void {
	el.innerHTML = s;
	el.outerHTML = s;
	el.insertAdjacentHTML('beforeend', s);
}
