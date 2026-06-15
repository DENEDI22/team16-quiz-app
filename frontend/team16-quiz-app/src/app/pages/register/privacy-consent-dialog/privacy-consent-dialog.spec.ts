import { ComponentFixture, TestBed } from '@angular/core/testing';

import { PrivacyConsentDialog } from './privacy-consent-dialog';

describe('PrivacyConsentDialog', () => {
  let component: PrivacyConsentDialog;
  let fixture: ComponentFixture<PrivacyConsentDialog>;

  beforeEach(async () => {
    await TestBed.configureTestingModule({
      imports: [PrivacyConsentDialog],
    }).compileComponents();

    fixture = TestBed.createComponent(PrivacyConsentDialog);
    component = fixture.componentInstance;
    await fixture.whenStable();
  });

  it('should create', () => {
    expect(component).toBeTruthy();
  });
});
