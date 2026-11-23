import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

import { ensureIntegrity, hasLowIntegrityLabel } from "./ensure-integrity.js";

function setIntegrityLevel(target: string, level: string): boolean {
    try {
        execFileSync("icacls", [target, "/setintegritylevel", level], { stdio: "ignore", windowsHide: true });
        return true;
    } catch {
        return false;
    }
}

test("hasLowIntegrityLabel 只认 Low 标签，Medium 与普通 ACL 都不算", () => {
    assert.equal(hasLowIntegrityLabel("Mandatory Label\\Low Mandatory Level:(I)(NW)"), true);
    assert.equal(hasLowIntegrityLabel("强制标签\\低强制级别:(I)(NW)"), true);
    assert.equal(hasLowIntegrityLabel("Mandatory Label\\Medium Mandatory Level:(I)(NW)"), false);
    assert.equal(hasLowIntegrityLabel("DESKTOP\\Tu1231:(I)(OI)(CI)(F)"), false);
    assert.equal(hasLowIntegrityLabel(""), false);
});

test("ensureIntegrity 把 Low 目录改回 Medium，正常目录不动", { skip: process.platform !== "win32" }, t => {
    const dir = mkdtempSync(join(tmpdir(), "integrity-"));
    t.after(() => rmSync(dir, { recursive: true, force: true, maxRetries: 3, retryDelay: 100 }));

    assert.equal(ensureIntegrity(dir), "ok", "新建目录本身是 Medium，不应被改动");

    // 模拟沙箱跑过之后的状态
    assert.equal(setIntegrityLevel(dir, "(OI)(CI)L"), true, "测试环境必须允许设置完整性标签");

    assert.equal(ensureIntegrity(dir), "repaired");
    assert.equal(ensureIntegrity(dir), "ok", "修好之后重复检查应当是 ok");
});
