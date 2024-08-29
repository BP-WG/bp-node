
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
            [CompletionResult]::new('--electrum', '--electrum', [CompletionResultType]::ParameterName, 'Electrum server to use')
            [CompletionResult]::new('--esplora', '--esplora', [CompletionResultType]::ParameterName, 'Esplora server to use')
            [CompletionResult]::new('--mempool', '--mempool', [CompletionResultType]::ParameterName, 'Mempool server to use')
            [CompletionResult]::new('-d', '-d', [CompletionResultType]::ParameterName, 'Data directory path')
            [CompletionResult]::new('--data-dir', '--data-dir', [CompletionResultType]::ParameterName, 'Data directory path')
            [CompletionResult]::new('-n', '-n', [CompletionResultType]::ParameterName, 'Network to use')
            [CompletionResult]::new('--network', '--network', [CompletionResultType]::ParameterName, 'Network to use')
            [CompletionResult]::new('-l', '-l', [CompletionResultType]::ParameterName, 'l')
            [CompletionResult]::new('--listen', '--listen', [CompletionResultType]::ParameterName, 'listen')
            [CompletionResult]::new('-v', '-v', [CompletionResultType]::ParameterName, 'Set verbosity level')
            [CompletionResult]::new('--verbose', '--verbose', [CompletionResultType]::ParameterName, 'Set verbosity level')
            [CompletionResult]::new('--no-network-prefix', '--no-network-prefix', [CompletionResultType]::ParameterName, 'Do not add network prefix to the `--data-dir`')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            [CompletionResult]::new('-V', '-V ', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('--version', '--version', [CompletionResultType]::ParameterName, 'Print version')
            break
        }
    })

    $completions.Where{ $_.CompletionText -like "$wordToComplete*" } |
        Sort-Object -Property ListItemText
}
