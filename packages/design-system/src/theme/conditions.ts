export const conditions = {
  extend: {
    // preset-panda's breakpoint-down conditions (the official Park preset
    // pairs with preset-panda; we re-declare the ones Park code relies on)
    smDown: '@media (max-width: 39.9975em)',
    mdDown: '@media (max-width: 47.9975em)',
    lgDown: '@media (max-width: 61.9975em)',
    xlDown: '@media (max-width: 79.9975em)',
    '2xlDown': '@media (max-width: 95.9975em)',
    light: ':root &, .light &',
    invalid: '&:is(:user-invalid, [data-invalid], [aria-invalid=true])',
    hover: '&:not(:disabled):hover',
    active: '&:not(:disabled):active',
    checked:
      '&:is(:checked, [data-checked], [data-state=checked], [aria-checked=true], [data-state=indeterminate])',
    on: '&:is([data-state=on])',
    pinned: '&:is([data-pinned])',
  },
} as const
