// Planroom sync probe: runs once per visit and records what this viewer's
// browser can do, so the session can read it back with ArtifactData.
const checks = [
  ['module', 'Same-origin module script runs'],
  ['fetch', 'Published data file loads by relative URL'],
  ['db', 'Shared store is available'],
  ['user', 'Viewer identity is available'],
  ['write', 'Edit lands in your own inbox'],
  ['ack', 'Server confirms the edit (round trip)'],
  ['head', 'Live head updates arrive from the session'],
]
const state = Object.fromEntries(checks.map(([k]) => [k, { status: 'pending', detail: 'Waiting' }]))

const list = document.getElementById('checks')
for (const [key, label] of checks) {
  const li = document.createElement('li')
  li.id = `check-${key}`
  li.innerHTML = '<span class="chip"></span><span class="label"></span><span class="detail"></span>'
  li.querySelector('.label').textContent = label
  list.append(li)
}

function mark(key, status, detail) {
  state[key] = { status, detail }
  const li = document.getElementById(`check-${key}`)
  li.dataset.status = status
  li.querySelector('.chip').textContent = { pending: 'Waiting', pass: 'Works', fail: 'Fails', skip: 'Skipped' }[status]
  li.querySelector('.detail').textContent = detail
}
checks.forEach(([k]) => mark(k, 'pending', 'Waiting'))
document.documentElement.dataset.module = 'ok'
mark('module', 'pass', 'assets/probe.js ran as a module')

let report = null
async function save() {
  if (!report) return
  try {
    await report.set({ format: 'planroom.probe/v1', at: Date.now(), checks: state, ua: navigator.userAgent.slice(0, 160) })
  } catch (e) {
    document.getElementById('note').textContent = `Couldn't save the results: ${e.code ?? e.message}`
  }
}

async function run() {
  try {
    const r = await fetch('./data/snapshot.json')
    const j = await r.json()
    mark('fetch', 'pass', `${j.nodes.length} nodes, ${j.edges.length} edges`)
  } catch (e) {
    mark('fetch', 'fail', String(e.message ?? e))
  }

  const db = await window.claude?.use('db')
  const user = await window.claude?.use('user')
  if (!db) {
    mark('db', 'fail', 'Not available in this view (signed out, or not granted)')
    ;['write', 'ack', 'head'].forEach(k => mark(k, 'skip', 'Needs the shared store'))
  } else {
    mark('db', 'pass', 'Ready')
  }
  const uid = user ? await user.id() : null
  const canWrite = user ? await user.can('data.write') : null
  mark('user', uid ? 'pass' : 'fail', uid ? `Signed in; may write shared data: ${canWrite === null ? 'not told' : canWrite}` : 'No viewer id')
  if (!db || !uid) return

  report = db.doc(`inbox/${uid}/probe/latest`)
  const events = db.collection(`inbox/${uid}/events`)
  const ev = events.doc()
  const sentAt = performance.now()
  let acked = false
  const stop = ev.onSnapshot(s => {
    if (acked || !s.exists || s.metadata.hasPendingWrites) return
    acked = true
    stop()
    mark('ack', 'pass', `${Math.round(performance.now() - sentAt)} ms from write to server confirmation`)
    save()
  }, e => mark('ack', 'fail', e.code))
  try {
    await db.doc(`inbox/${uid}`).set({ seq: 1, updatedAt: Date.now() })
    await ev.set({ seq: 1, kind: 'probe', idempotencyKey: ev.id, baseVersion: 1, at: Date.now() })
    mark('write', 'pass', `inbox/${uid.slice(0, 6)}…/events/${ev.id.slice(0, 6)}…`)
  } catch (e) {
    mark('write', 'fail', e.code ?? String(e))
  }

  let seen = 0
  db.doc('live/head').onSnapshot(s => {
    if (!s.exists) {
      mark('head', 'pending', 'No head yet. The session writes one when it next syncs.')
      return
    }
    const d = s.data()
    seen += 1
    const lag = typeof d.sentAt === 'number' ? Date.now() - d.sentAt : null
    mark('head', 'pass', `head ${d.head}, ${seen} update${seen === 1 ? '' : 's'} seen${lag === null ? '' : `, ${lag} ms after the session sent it`}`)
    save()
  }, e => mark('head', 'fail', e.code))
  save()
}

run()
