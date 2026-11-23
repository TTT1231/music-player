import { cleanup, fireEvent, render } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { Cover } from "./Cover";

afterEach(cleanup);

test("shows the embedded image with anonymous access for extracting its color", () => {
    const { container } = render(<Cover id="song" title="歌曲" coverUrl="http://cover.localhost/song" size={64} />);
    const image = container.querySelector("img")!;
    expect(image.getAttribute("src")).toBe("http://cover.localhost/song");
    expect(image.getAttribute("crossorigin")).toBe("anonymous");
    expect(image.style.getPropertyValue("--s")).toBe("64px");
});

test("uses the song title glyph and a stable color when no cover can be read", () => {
    const { container, rerender } = render(<Cover id="song" title="...歌曲" coverUrl="http://cover.localhost/song" />);
    fireEvent.error(container.querySelector("img")!);
    const fallback = container.querySelector(".cover") as HTMLElement;
    expect(fallback.textContent).toBe("歌");
    const color = fallback.style.getPropertyValue("--bg");
    expect(color).not.toBe("");
    rerender(<Cover id="song" title="新标题" />);
    expect((container.querySelector(".cover") as HTMLElement).style.getPropertyValue("--bg")).toBe(color);
    expect(container.textContent).toBe("新");
    rerender(<Cover id="another" title="!!!" />);
    expect(container.textContent).toBe("♪");
});

test("uses the given colors for built-in covers instead of hashing the id", () => {
    const { container } = render(<Cover id="all" title="全部音乐" size={56} colors={["#2440b8", "#eef0ea"]} />);
    const cover = container.querySelector(".cover") as HTMLElement;
    expect(cover.style.getPropertyValue("--bg")).toBe("#2440b8");
    expect(cover.style.getPropertyValue("--fg")).toBe("#eef0ea");
    expect(cover.textContent).toBe("全");
});

test("retries a missing cover when the same song is reimported", () => {
    const url = "http://cover.localhost/song";
    const { container, rerender } = render(<Cover id="song" title="歌" coverUrl={url} revision={1} />);
    const original = container.querySelector("img")!;
    expect(original.getAttribute("src")).toBe(`${url}?coverRevision=1`);
    fireEvent.error(original);
    expect(container.querySelector("img")).toBeNull();
    rerender(<Cover id="song" title="歌" coverUrl={url} revision={1} />);
    expect(container.querySelector("img")).toBeNull();
    rerender(<Cover id="song" title="歌" coverUrl={url} revision={2} />);
    expect(container.querySelector("img")?.getAttribute("src")).toBe(`${url}?coverRevision=2`);
});

test("requests a fresh image on reimport without discarding existing query or fragment", () => {
    const url = "http://cover.localhost/song?existing=1#art";
    const { container, rerender } = render(<Cover id="song" title="歌" coverUrl={url} revision="first value" />);
    const original = container.querySelector("img");
    expect(original?.getAttribute("src")).toBe(
        "http://cover.localhost/song?existing=1&coverRevision=first%20value#art",
    );
    rerender(<Cover id="song" title="歌" coverUrl={url} revision="next" />);
    expect(container.querySelector("img")).not.toBe(original);
    expect(container.querySelector("img")?.getAttribute("src")).toBe(
        "http://cover.localhost/song?existing=1&coverRevision=next#art",
    );
});
