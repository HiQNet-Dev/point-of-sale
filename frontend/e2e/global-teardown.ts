import { rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

export default async function teardown() {
  await rm(join(tmpdir(), 'hiqnet-pos-e2e-fixture.json'), { force: true })
}
