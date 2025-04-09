
using namespace System.Management.Automation
using namespace System.Management.Automation.Language

Register-ArgumentCompleter -Native -CommandName 'bp-node' -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)

    $commandElements = $commandAst.CommandElements
    $command = @(
        'bp-node'
        for ($i = 1; $i -lt $commandElements.Count; $i++) {
            $element = $commandElements[$i]
            if ($element -isnot [StringConstantExpressionAst] -or
                $element.StringConstantType -ne [StringConstantType]::BareWord -or
                $element.Value.StartsWith('-') -or
                $element.Value -eq $wordToComplete) {
                break
        }
        $element.Value
    }) -join ';'

    $completions = @(switch ($command) {
        'bp-node' {
            [CompletionResult]::new('-d', '-d', [CompletionResultType]::ParameterName, 'Data directory path')
            [CompletionResult]::new('--data-dir', '--data-dir', [CompletionResultType]::ParameterName, 'Data directory path')
            [CompletionResult]::new('-n', '-n', [CompletionResultType]::ParameterName, 'Network to use')
            [CompletionResult]::new('--network', '--network', [CompletionResultType]::ParameterName, 'Network to use')
            [CompletionResult]::new('-l', '-l', [CompletionResultType]::ParameterName, 'l')
            [CompletionResult]::new('--listen', '--listen', [CompletionResultType]::ParameterName, 'listen')
            [CompletionResult]::new('-p', '-p', [CompletionResultType]::ParameterName, 'p')
            [CompletionResult]::new('--provider', '--provider', [CompletionResultType]::ParameterName, 'provider')
            [CompletionResult]::new('-v', '-v', [CompletionResultType]::ParameterName, 'Set verbosity level')
            [CompletionResult]::new('--verbose', '--verbose', [CompletionResultType]::ParameterName, 'Set verbosity level')
            [CompletionResult]::new('--no-network-prefix', '--no-network-prefix', [CompletionResultType]::ParameterName, 'Do not add network prefix to the `--data-dir`')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            [CompletionResult]::new('-V', '-V ', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('--version', '--version', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('init', 'init', [CompletionResultType]::ParameterValue, 'init')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'bp-node;init' {
            [CompletionResult]::new('-d', '-d', [CompletionResultType]::ParameterName, 'Data directory path')
            [CompletionResult]::new('--data-dir', '--data-dir', [CompletionResultType]::ParameterName, 'Data directory path')
            [CompletionResult]::new('-n', '-n', [CompletionResultType]::ParameterName, 'Network to use')
            [CompletionResult]::new('--network', '--network', [CompletionResultType]::ParameterName, 'Network to use')
            [CompletionResult]::new('-v', '-v', [CompletionResultType]::ParameterName, 'Set verbosity level')
            [CompletionResult]::new('--verbose', '--verbose', [CompletionResultType]::ParameterName, 'Set verbosity level')
            [CompletionResult]::new('--no-network-prefix', '--no-network-prefix', [CompletionResultType]::ParameterName, 'Do not add network prefix to the `--data-dir`')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            break
        }
        'bp-node;help' {
            [CompletionResult]::new('init', 'init', [CompletionResultType]::ParameterValue, 'init')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'bp-node;help;init' {
            break
        }
        'bp-node;help;help' {
            break
        }
    })

    $completions.Where{ $_.CompletionText -like "$wordToComplete*" } |
        Sort-Object -Property ListItemText
}
