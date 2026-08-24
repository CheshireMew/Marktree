import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

const root = new URL('../', import.meta.url)
const read = (path) => readFile(new URL(path, root), 'utf8')
const policy = JSON.parse(await read('license-policy.json'))
const [
  packageSource,
  packageLockSource,
  cargo,
  readme,
  licensing,
  current,
  currentNotice,
] =
  await Promise.all([
    read('package.json'),
    read('package-lock.json'),
    read('src-tauri/Cargo.toml'),
    read('README.md'),
    read('LICENSING.md'),
    read(policy.current.licenseFile),
    read(policy.current.noticeFile),
  ])

const packageManifest = JSON.parse(packageSource)
const packageLock = JSON.parse(packageLockSource)
assert.equal(packageManifest.license, policy.current.spdx)
assert.equal(packageLock.packages[''].license, policy.current.spdx)
assert.match(cargo, new RegExp(`^license = "${policy.current.spdx}"$`, 'm'))
assert.ok(readme.includes(`[${policy.current.spdx}](LICENSING.md)`))
assert.ok(licensing.includes(`\`${policy.current.spdx}\``))
assert.ok(!licensing.includes('$spdx'))
assert.ok(!licensing.includes('$marktreeCutoff'))
assert.ok(current.includes('GNU AFFERO GENERAL PUBLIC LICENSE'))
assert.ok(currentNotice.includes(policy.current.spdx))

for (const grant of policy.historicalGrants) {
  const license = await read(grant.licenseFile)
  assert.ok(readme.includes(grant.lastCommit))
  assert.ok(licensing.includes(grant.lastCommit))
  if (grant.spdx === 'GPL-3.0-or-later') {
    assert.ok(license.includes('GNU GENERAL PUBLIC LICENSE'))
  } else if (grant.spdx === 'MIT') {
    assert.ok(license.includes('MIT License'))
  } else {
    assert.fail(`Unsupported historical license: ${grant.spdx}`)
  }
  if (grant.noticeFile) {
    const notice = await read(grant.noticeFile)
    assert.ok(notice.includes('Licensing'))
  }
}
