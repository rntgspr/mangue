// Parses a .bpmn file with bpmn-moddle and exits non-zero on any warning or error.
import { readFile } from 'node:fs/promises'
import { BpmnModdle } from 'bpmn-moddle'

/** Reads the file given as argv[2], parses it and reports problems. */
async function main() {
  const file = process.argv[2]

  if (!file) {
    console.error('usage: node validate.mjs <file.bpmn>')
    process.exit(2)
  }

  const xml = await readFile(file, 'utf8')

  try {
    const { warnings } = await new BpmnModdle().fromXML(xml)

    if (warnings.length > 0) {
      for (const w of warnings) console.error(`warning: ${w.message}`)
      process.exit(1)
    }
  } catch (err) {
    console.error(`error: ${err.message}`)
    process.exit(1)
  }

  console.log(`ok: ${file}`)
}

main()
