#!/usr/bin/env node

import { Command } from 'commander'
import { init } from './index'

const program = new Command()
  .name('rustier')
  .description('Install and configure Rustier in a project')

program
  .command('init')
  .aliases(['i', 'create', 'scaffold', 'setup'])
  .description('Initialize Rustier in a project (hello-world placeholder)')
  .option('-d, --dir <path>', 'Project directory')
  .action(init)

await program.parseAsync(process.argv)
