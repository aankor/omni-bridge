import {Command} from 'commander';
import {setupContext} from './context';
import {installInitializeCLI} from './initialize';

export function cli() {
  const program = new Command();

  program
    .version('0.0.1')
    .allowExcessArguments(false)
    .option('--key-map <string>', 'Path to the key map', 'keyMap.json')
    .option('--config <string>', 'Config')
    .option('--cluster <string>', 'Cluster name or endpoint address')
    .option('--wallet <string>', 'Path to the signer keypair')
    .option('--commitment <string>', 'Commitment level')
    .option('--skip-preflight', 'Skip preflight')
    .option('--lookup-table <pubkey>', 'Lookup table address')
    .option('--simulate', 'Run simulation first')
    .option('--print <multisig|legacy|0>', 'Print tx instead of running')
    .hook('preAction', (command: Command) => setupContext(command.opts()));

  installInitializeCLI(program);

  return program;
}
